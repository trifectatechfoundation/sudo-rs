use std::{
    ffi::OsString,
    fs, io,
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::Path,
};

use crate::system::{Process, WithProcess, term::Terminal};

pub(super) fn ttyname_from_dev() -> io::Result<Option<OsString>> {
    let Ok(Some(tty_dev)) = Process::tty_device_id(WithProcess::Current) else {
        return Ok(None);
    };

    let tty_dev = tty_dev.inner();

    let tty_name = dev_check(Path::new("/dev/console"), tty_dev)
        .or_else(|| ttyname_from_stdioe(tty_dev))
        .or_else(|| find_tty_in_dir(Path::new("/dev/pts"), tty_dev))
        .or_else(|| find_tty_in_dir(Path::new("/dev"), tty_dev));

    if tty_name.is_some() {
        Ok(tty_name)
    } else {
        Err(io::ErrorKind::NotFound.into())
    }
}

fn is_our_tty(metadata: fs::Metadata, tty_dev: libc::dev_t) -> bool {
    metadata.file_type().is_char_device() && metadata.rdev() == tty_dev
}

fn ttyname_from_stdioe(tty_dev: libc::dev_t) -> Option<OsString> {
    [
        io::stdin().ttyname(),
        io::stdout().ttyname(),
        io::stderr().ttyname(),
    ]
    .iter()
    .flatten()
    .find_map(|ttyname| dev_check(ttyname.as_ref(), tty_dev))
}

fn dev_check(path: &Path, tty_dev: libc::dev_t) -> Option<OsString> {
    let metadata = fs::metadata(path).ok()?;

    if is_our_tty(metadata, tty_dev) {
        Some(path.into())
    } else {
        None
    }
}

fn find_tty_in_dir(dir: &Path, tty_dev: libc::dev_t) -> Option<OsString> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        if let Ok(metadata) = entry.metadata() {
            if is_our_tty(metadata, tty_dev) {
                return Some(entry.path().into());
            }
        }
    }

    None
}
