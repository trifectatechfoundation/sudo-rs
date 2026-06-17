use std::{
    ffi::OsString,
    fs,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

use crate::system::{Process, WithProcess};

pub(super) fn ttyname_from_dev() -> Option<OsString> {
    let tty_dev = Process::tty_device_id(WithProcess::Current)
        .ok()
        .flatten()?
        .inner();

    dev_check(Path::new("/dev/console"), tty_dev)
        .or_else(|| ttyname_from_proc_self_fd(tty_dev))
        .or_else(|| find_tty_in_dir(Path::new("/dev/pts"), tty_dev))
        .or_else(|| find_tty_in_dir(Path::new("/dev"), tty_dev))
}

fn ttyname_from_proc_self_fd(tty_dev: libc::dev_t) -> Option<OsString> {
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
            return Some(path.into_os_string());
        }
    }
    None
}

fn dev_check(path: &Path, tty_dev: libc::dev_t) -> Option<OsString> {
    let metadata = fs::metadata(path).ok()?;

    if metadata.file_type().is_char_device() && metadata.rdev() == tty_dev {
        Some(path.as_os_str().to_os_string())
    } else {
        None
    }
}

fn find_tty_in_dir(dir: &Path, tty_dev: libc::dev_t) -> Option<OsString> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        if let Ok(metadata) = entry.metadata() {
            if metadata.file_type().is_char_device() && metadata.rdev() == tty_dev {
                return Some(entry.path().into_os_string());
            }
        }
    }

    None
}
