#![allow(unsafe_code)]

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::net::Shutdown;
use std::os::unix::{fs::OpenOptionsExt, net::UnixStream, process::ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{io, process};

use crate::exec::ExitReason;
use crate::system::file::{create_temporary_dir, FileLock};
use crate::system::wait::{Wait, WaitError, WaitOptions};
use crate::system::{fork, ForkResult};

struct ParentFileInfo<'a> {
    path: &'a Path,
    file: File,
    lock: FileLock,
    old_data: Vec<u8>,
    new_data_rx: UnixStream,
    new_data: Option<Vec<u8>>,
}

struct ChildFileInfo<'a> {
    path: &'a Path,
    old_data: Vec<u8>,
    tempfile_path: Option<PathBuf>,
    new_data_tx: UnixStream,
}

pub(super) fn edit_files(editor: &Path, paths: &[&Path]) -> io::Result<ExitReason> {
    let mut files = vec![];
    let mut child_files = vec![];
    for path in paths {
        // Open file
        let mut file: File = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|e| {
                io::Error::new(e.kind(), format!("Failed to open {}: {e}", path.display()))
            })?;

        // Error for special files
        let metadata = file.metadata().map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("Failed to read metadata for {}: {e}", path.display()),
            )
        })?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("File {} is not a regular file", path.display()),
            ));
        }

        // Take file lock
        let lock = FileLock::exclusive(&file, true).map_err(|e| {
            io::Error::new(e.kind(), format!("Failed to lock {}: {e}", path.display()))
        })?;

        // Read file
        let mut old_data = Vec::new();
        file.read_to_end(&mut old_data).map_err(|e| {
            io::Error::new(e.kind(), format!("Failed to read {}: {e}", path.display()))
        })?;

        // Create socket
        let (parent_socket, child_socket) = UnixStream::pair().unwrap();

        files.push(ParentFileInfo {
            path,
            file,
            lock,
            old_data: old_data.clone(),
            new_data_rx: parent_socket,
            new_data: None,
        });

        child_files.push(ChildFileInfo {
            path,
            old_data,
            tempfile_path: None,
            new_data_tx: child_socket,
        });
    }

    // Spawn child
    // SAFETY: There should be no other threads at this point.
    let ForkResult::Parent(command_pid) = unsafe { fork() }.unwrap() else {
        drop(files);
        handle_child(editor, child_files)
    };
    drop(child_files);

    for file in &mut files {
        // Read from socket
        file.new_data =
            Some(read_stream(&mut file.new_data_rx).map_err(|e| {
                io::Error::new(e.kind(), format!("Failed to read from socket: {e}"))
            })?);
    }

    // If child has error, exit with non-zero exit code
    let status = loop {
        match command_pid.wait(WaitOptions::new()) {
            Ok((_, status)) => break status,
            Err(WaitError::Io(err)) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => panic!("{err:?}"),
        }
    };
    assert!(status.did_exit());
    if let Some(signal) = status.term_signal() {
        return Ok(ExitReason::Signal(signal));
    } else if let Some(code) = status.exit_status() {
        if code != 0 {
            return Ok(ExitReason::Code(code));
        }
    } else {
        return Ok(ExitReason::Code(1));
    }

    for mut file in files {
        let data = file.new_data.expect("filled in above");
        if data == file.old_data {
            // File unchanged. No need to write it again.
            continue;
        }

        // FIXME check if modified since reading and if so ask user what to do

        // Write file
        (move || {
            file.file.rewind()?;
            file.file.write_all(&data)?;
            file.file.set_len(
                data.len()
                    .try_into()
                    .expect("more than 18 exabyte of data???"),
            )
        })()
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("Failed to write {}: {e}", file.path.display()),
            )
        })?;

        drop(file.lock);
    }

    Ok(ExitReason::Code(0))
}

struct TempDirDropGuard(PathBuf);

impl Drop for TempDirDropGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir(&self.0) {
            eprintln_ignore_io_error!(
                "Failed to remove temporary directory {}: {e}",
                self.0.display(),
            );
        };
    }
}

fn handle_child(editor: &Path, file: Vec<ChildFileInfo<'_>>) -> ! {
    match handle_child_inner(editor, file) {
        Ok(()) => process::exit(0),
        Err(err) => {
            eprintln_ignore_io_error!("{err}");
            process::exit(1);
        }
    }
}

// FIXME maybe use pipes once std::io::pipe has been stabilized long enough.
fn handle_child_inner(editor: &Path, mut files: Vec<ChildFileInfo<'_>>) -> Result<(), String> {
    // Drop root privileges.
    // SAFETY: setuid does not change any memory and only affects OS state.
    unsafe {
        libc::setuid(libc::getuid());
    }

    let tempdir = TempDirDropGuard(
        create_temporary_dir().map_err(|e| format!("Failed to create temporary directory: {e}"))?,
    );

    for file in &mut files {
        // Create temp file
        let tempfile_path = tempdir
            .0
            .join(file.path.file_name().expect("file must have filename"));
        let mut tempfile = std::fs::File::create_new(&tempfile_path).map_err(|e| {
            format!(
                "Failed to create temporary file {}: {e}",
                tempfile_path.display(),
            )
        })?;

        // Write to temp file
        tempfile.write_all(&file.old_data).map_err(|e| {
            format!(
                "Failed to write to temporary file {}: {e}",
                tempfile_path.display(),
            )
        })?;
        drop(tempfile);
        file.tempfile_path = Some(tempfile_path);
    }

    // Spawn editor
    let status = Command::new(editor)
        .args(
            files
                .iter()
                .map(|file| file.tempfile_path.as_ref().expect("filled in above")),
        )
        .status()
        .map_err(|e| format!("Failed to run editor {}: {e}", editor.display()))?;
    if !status.success() {
        drop(tempdir);

        if let Some(signal) = status.signal() {
            process::exit(128 + signal);
        }
        process::exit(status.code().unwrap_or(1));
    }

    for mut file in files {
        let tempfile_path = file.tempfile_path.as_ref().expect("filled in above");

        // Read from temp file
        let new_data = std::fs::read(&tempfile_path).map_err(|e| {
            format!(
                "Failed to read from temporary file {}: {e}",
                tempfile_path.display(),
            )
        })?;

        // FIXME preserve temporary file if the original couldn't be written to
        std::fs::remove_file(&tempfile_path).map_err(|e| {
            format!(
                "Failed to remove temporary file {}: {e}",
                tempfile_path.display(),
            )
        })?;

        // If the file has been changed to be empty, ask the user what to do.
        if new_data.is_empty() && new_data != file.old_data {
            match crate::visudo::ask_response(
                format!(
                    "sudoedit: truncate {} to zero? (y/n) [n] ",
                    file.path.display()
                )
                .as_bytes(),
                b"yn",
            ) {
                Ok(b'y') => {}
                _ => {
                    eprintln_ignore_io_error!("Not overwriting {}", file.path.display());

                    // Parent ignores write when new data matches old data
                    write_stream(&mut file.new_data_tx, &file.old_data)
                        .map_err(|e| format!("Failed to write data to parent: {e}"))?;

                    continue;
                }
            }
        }

        // Write to socket
        write_stream(&mut file.new_data_tx, &new_data)
            .map_err(|e| format!("Failed to write data to parent: {e}"))?;
    }

    process::exit(0);
}

fn write_stream(socket: &mut UnixStream, data: &[u8]) -> io::Result<()> {
    socket.write_all(data)?;
    socket.shutdown(Shutdown::Both)?;
    Ok(())
}

fn read_stream(socket: &mut UnixStream) -> io::Result<Vec<u8>> {
    let mut new_data = Vec::new();
    socket.read_to_end(&mut new_data)?;
    Ok(new_data)
}
