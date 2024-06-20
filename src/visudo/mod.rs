mod cli;
mod help;

use std::{
    ffi::{CString, OsString},
    fs::{File, Permissions},
    io::{self, Read, Seek, Write},
    os::unix::prelude::{MetadataExt, OsStringExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    sudo::diagnostic,
    sudoers::Sudoers,
    system::{
        can_execute,
        file::{Chown, FileLock},
        signal::{consts::*, register_handlers, SignalStream},
        User,
    },
};

use self::cli::{VisudoAction, VisudoOptions};
use self::help::{long_help_message, USAGE_MSG};

const VERSION: &str = env!("CARGO_PKG_VERSION");

macro_rules! io_msg {
    ($err:expr, $($tt:tt)*) => {
        io::Error::new($err.kind(), format!("{}: {}", format_args!($($tt)*), $err))
    };
}

pub fn main() {
    let options = match VisudoOptions::from_env() {
        Ok(options) => options,
        Err(error) => {
            println_ignore_io_error!("visudo: {error}\n{USAGE_MSG}");
            std::process::exit(1);
        }
    };

    let cmd = match options.action {
        VisudoAction::Help => {
            println_ignore_io_error!("{}", long_help_message());
            std::process::exit(0);
        }
        VisudoAction::Version => {
            println_ignore_io_error!("visudo version {VERSION}");
            std::process::exit(0);
        }
        VisudoAction::Check => check,
        VisudoAction::Run => run,
    };

    match cmd(options.file.as_deref(), options.perms, options.owner) {
        Ok(()) => {}
        Err(error) => {
            eprintln_ignore_io_error!("visudo: {error}");
            std::process::exit(1);
        }
    }
}

fn check(file_arg: Option<&str>, perms: bool, owner: bool) -> io::Result<()> {
    let sudoers_path = Path::new(file_arg.unwrap_or("/etc/sudoers"));

    let sudoers_file = File::open(sudoers_path)
        .map_err(|err| io_msg!(err, "unable to open {}", sudoers_path.display()))?;

    let metadata = sudoers_file.metadata()?;

    if file_arg.is_none() || perms {
        // For some reason, the MSB of the mode is on so we need to mask it.
        let mode = metadata.permissions().mode() & 0o777;

        if mode != 0o440 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "{}: bad permissions, should be mode 0440, but found {mode:04o}",
                    sudoers_path.display()
                ),
            ));
        }
    }

    if file_arg.is_none() || owner {
        let owner = (metadata.uid(), metadata.gid());

        if owner != (0, 0) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "{}: wrong owner (uid, gid) should be (0, 0), but found {owner:?}",
                    sudoers_path.display()
                ),
            ));
        }
    }

    let (_sudoers, errors) = Sudoers::read(&sudoers_file, sudoers_path)?;

    if errors.is_empty() {
        writeln!(io::stdout(), "{}: parsed OK", sudoers_path.display())?;
        return Ok(());
    }

    for crate::sudoers::Error {
        message,
        source,
        location,
    } in errors
    {
        let path = source.as_deref().unwrap_or(sudoers_path);
        diagnostic::diagnostic!("syntax error: {message}", path @ location);
    }

    Err(io::Error::new(io::ErrorKind::Other, "invalid sudoers file"))
}

fn run(file_arg: Option<&str>, perms: bool, owner: bool) -> io::Result<()> {
    let sudoers_path = Path::new(file_arg.unwrap_or("/etc/sudoers"));

    let (sudoers_file, existed) = if sudoers_path.exists() {
        let file = File::options().read(true).write(true).open(sudoers_path)?;

        (file, true)
    } else {
        // Create a sudoers file if it doesn't exist.
        let file = File::create(sudoers_path)?;
        // ogvisudo sets the permissions of the file so it can be read and written by the user and
        // read by the group if the `-f` argument was passed.
        if file_arg.is_some() {
            file.set_permissions(Permissions::from_mode(0o640))?;
        }
        (file, false)
    };

    let lock = FileLock::exclusive(&sudoers_file, true).map_err(|err| {
        if err.kind() == io::ErrorKind::WouldBlock {
            io_msg!(err, "{} busy, try again later", sudoers_path.display())
        } else {
            err
        }
    })?;

    if perms || file_arg.is_none() {
        sudoers_file.set_permissions(Permissions::from_mode(0o440))?;
    }

    if owner || file_arg.is_none() {
        sudoers_file.chown(User::real_uid(), User::real_gid())?;
    }

    let signal_stream = SignalStream::init()?;

    let handlers = register_handlers([SIGTERM, SIGHUP, SIGINT, SIGQUIT])?;

    let tmp_dir = create_temporary_dir()?;
    let tmp_path = tmp_dir.join("sudoers");

    {
        let tmp_dir = tmp_dir.clone();
        std::thread::spawn(|| -> io::Result<()> {
            signal_stream.recv()?;

            let _ = std::fs::remove_dir_all(tmp_dir);

            drop(handlers);

            std::process::exit(1)
        });
    }

    let tmp_file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp_path)?;

    tmp_file.set_permissions(Permissions::from_mode(0o700))?;

    let result = edit_sudoers_file(
        existed,
        sudoers_file,
        sudoers_path,
        lock,
        tmp_file,
        &tmp_path,
    );

    std::fs::remove_dir_all(tmp_dir)?;

    result
}

fn edit_sudoers_file(
    existed: bool,
    mut sudoers_file: File,
    sudoers_path: &Path,
    lock: FileLock,
    mut tmp_file: File,
    tmp_path: &Path,
) -> io::Result<()> {
    let mut editor_path = None;
    let mut sudoers_contents = Vec::new();
    if existed {
        // If the sudoers file existed, read its contents and write them into the temporary file.
        sudoers_file.read_to_end(&mut sudoers_contents)?;
        // Rewind the sudoers file so it can be written later.
        sudoers_file.rewind()?;
        // Write to the temporary file.
        tmp_file.write_all(&sudoers_contents)?;

        let (sudoers, errors) = Sudoers::read(sudoers_contents.as_slice(), sudoers_path)?;

        if errors.is_empty() {
            editor_path = sudoers.solve_editor_path();
        }
    }

    let editor_path = match editor_path {
        Some(path) => path,
        None => editor_path_fallback()?,
    };

    let mut stderr = io::stderr();
    loop {
        Command::new(&editor_path)
            .arg("--")
            .arg(tmp_path)
            .spawn()?
            .wait_with_output()?;

        let (_sudoers, errors) = File::open(tmp_path)
            .and_then(|reader| Sudoers::read(reader, tmp_path))
            .map_err(|err| {
                io_msg!(
                    err,
                    "unable to re-open temporary file ({}), {} unchanged",
                    tmp_path.display(),
                    sudoers_path.display()
                )
            })?;

        if errors.is_empty() {
            break;
        }

        writeln!(stderr, "The provided sudoers file format is not recognized or contains syntax errors. Please review:\n")?;

        for crate::sudoers::Error { message, .. } in errors {
            writeln!(stderr, "syntax error: {message}")?;
        }

        writeln!(stderr)?;

        let stdin = io::stdin();
        let stdout = io::stdout();

        let mut stdin_handle = stdin.lock();
        let mut stdout_handle = stdout.lock();

        loop {
            stdout_handle
                .write_all("What now? e(x)it without saving / (e)dit again: ".as_bytes())?;
            stdout_handle.flush()?;

            let mut input = [0u8];
            if let Err(err) = stdin_handle.read_exact(&mut input) {
                writeln!(stderr, "visudo: cannot read user input: {err}")?;
                return Ok(());
            }

            match &input {
                b"e" => break,
                b"x" => return Ok(()),
                input => writeln!(stderr, "Invalid option: {:?}\n", std::str::from_utf8(input))?,
            }
        }
    }

    let tmp_contents = std::fs::read(tmp_path)?;
    // Only write to the sudoers file if the contents changed.
    if tmp_contents == sudoers_contents {
        writeln!(stderr, "visudo: {} unchanged", tmp_path.display())?;
    } else {
        sudoers_file.write_all(&tmp_contents)?;
        let new_size = sudoers_file.stream_position()?;
        sudoers_file.set_len(new_size)?;
    }

    lock.unlock()?;

    Ok(())
}

fn editor_path_fallback() -> io::Result<PathBuf> {
    let path = Path::new("/usr/bin/editor");
    if can_execute(path) {
        return Ok(path.to_owned());
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "cannot find text editor",
    ))
}

fn create_temporary_dir() -> io::Result<PathBuf> {
    let template = cstr!("/tmp/sudoers-XXXXXX").to_owned();

    let ptr = unsafe { libc::mkdtemp(template.into_raw()) };

    if ptr.is_null() {
        return Err(io::Error::last_os_error());
    }

    let path = OsString::from_vec(unsafe { CString::from_raw(ptr) }.into_bytes()).into();

    Ok(path)
}
