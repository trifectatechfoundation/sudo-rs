use std::ffi::{CString, OsString};
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

pub(crate) fn create_temporary_dir() -> io::Result<PathBuf> {
    let template = c"/tmp/sudoers-XXXXXX".to_owned();

    // SAFETY: mkdtemp is passed a valid null-terminated C string
    let ptr = unsafe { libc::mkdtemp(template.into_raw()) };

    if ptr.is_null() {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: ptr is the same pointer produced by into_raw() above, and it
    // is still pointing to a zero-terminated C string
    let path = OsString::from_vec(unsafe { CString::from_raw(ptr) }.into_bytes()).into();

    Ok(path)
}
