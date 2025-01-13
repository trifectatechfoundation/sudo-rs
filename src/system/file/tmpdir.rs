use std::ffi::{CString, OsString};
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

pub(crate) fn create_temporary_dir() -> io::Result<PathBuf> {
    let template = cstr!("/tmp/sudoers-XXXXXX").to_owned();

    let ptr = unsafe { libc::mkdtemp(template.into_raw()) };

    if ptr.is_null() {
        return Err(io::Error::last_os_error());
    }

    let path = OsString::from_vec(unsafe { CString::from_raw(ptr) }.into_bytes()).into();

    Ok(path)
}
