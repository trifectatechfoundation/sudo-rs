use std::{
    ffi::OsString,
    fs, io,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

use crate::system::{Process, WithProcess};

pub(super) fn ttyname_from_dev() -> io::Result<Option<OsString>> {
    let Ok(Some(tty_dev)) = Process::tty_device_id(WithProcess::Current) else {
        return Ok(None);
    };

    let tty_dev = tty_dev.inner();
    if let Some(tty_name) = ttyname_from_proc_self_fd(tty_dev)? {
        return Ok(Some(tty_name));
    }
    if let Some(tty_name) = dev_check(Path::new("/dev/console"), tty_dev)? {
        return Ok(Some(tty_name));
    }
    if let Some(tty_name) = find_tty_in_dir(Path::new("/dev/pts"), tty_dev)? {
        return Ok(Some(tty_name));
    }
    find_tty_in_dir(Path::new("/dev"), tty_dev)
}

fn ttyname_from_proc_self_fd(tty_dev: libc::dev_t) -> io::Result<Option<OsString>> {
    for fd in libc::STDIN_FILENO..=libc::STDERR_FILENO {
        let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: `st` points to uninitialized memory for libc to write the stat struct.
        if unsafe { libc::fstat(fd, st.as_mut_ptr()) } != 0 {
            continue;
        }
        // SAFETY: fstat() succeeded.
        let st = unsafe { st.assume_init() };
        if (st.st_mode & libc::S_IFMT) != libc::S_IFCHR || st.st_rdev != tty_dev {
            continue;
        }
        let link = PathBuf::from(format!("/proc/self/fd/{fd}"));
        if let Ok(path) = fs::read_link(link) {
            return Ok(Some(path.into_os_string()));
        }
    }
    Ok(None)
}

fn dev_check(path: &Path, tty_dev: libc::dev_t) -> io::Result<Option<OsString>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if metadata.file_type().is_char_device() && metadata.rdev() == tty_dev {
        return Ok(Some(path.as_os_str().to_os_string()));
    }
    Ok(None)
}

fn find_tty_in_dir(dir: &Path, tty_dev: libc::dev_t) -> io::Result<Option<OsString>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.file_type().is_char_device() && metadata.rdev() == tty_dev {
            return Ok(Some(entry.path().into_os_string()));
        }
    }

    Ok(None)
}
