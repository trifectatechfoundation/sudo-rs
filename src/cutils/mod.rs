use std::{
    ffi::{CStr, OsStr, OsString},
    os::unix::prelude::OsStrExt,
};

pub fn cerr<Int: Copy + TryInto<libc::c_long>>(res: Int) -> std::io::Result<Int> {
    match res.try_into() {
        Ok(-1) => Err(std::io::Error::last_os_error()),
        _ => Ok(res),
    }
}

extern "C" {
    #[cfg_attr(
        any(target_os = "macos", target_os = "ios", target_os = "freebsd"),
        link_name = "__error"
    )]
    #[cfg_attr(
        any(target_os = "openbsd", target_os = "netbsd", target_os = "android"),
        link_name = "__errno"
    )]
    #[cfg_attr(target_os = "linux", link_name = "__errno_location")]
    fn errno_location() -> *mut libc::c_int;
}

pub fn set_errno(no: libc::c_int) {
    unsafe { *errno_location() = no };
}

pub fn sysconf(name: libc::c_int) -> Option<libc::c_long> {
    set_errno(0);
    cerr(unsafe { libc::sysconf(name) }).ok()
}

/// Create a Rust string copy from a C string pointer
/// WARNING: This uses `to_string_lossy` so should not be used for data where
/// information loss is unacceptable (use `os_string_from_ptr` instead)
///
/// # Safety
/// This function assumes that the pointer is either a null pointer or that
/// it points to a valid NUL-terminated C string.
pub unsafe fn string_from_ptr(ptr: *const libc::c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        let cstr = unsafe { CStr::from_ptr(ptr) };
        cstr.to_string_lossy().to_string()
    }
}

/// Create an `OsString` copy from a C string pointer.
///
/// # Safety
/// This function assumes that the pointer is either a null pointer or that
/// it points to a valid NUL-terminated C string.
pub unsafe fn os_string_from_ptr(ptr: *const libc::c_char) -> OsString {
    if ptr.is_null() {
        OsString::new()
    } else {
        let cstr = unsafe { CStr::from_ptr(ptr) };
        OsStr::from_bytes(cstr.to_bytes()).to_owned()
    }
}

/// Rust's standard library IsTerminal just directly calls isatty, which
/// we don't want since this performs IOCTL calls on them and file descriptors are under
/// the control of the user; so this checks if they are a character device first.
pub fn safe_isatty(fildes: libc::c_int) -> bool {
    // The Rust standard library doesn't have FileTypeExt on Std{in,out,err}, so we
    // can't just use FileTypeExt::is_char_device and have to resort to libc::fstat.
    let mut maybe_stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fildes, maybe_stat.as_mut_ptr()) } == 0 {
        let mode = unsafe { maybe_stat.assume_init() }.st_mode;

        // To complicate matters further, the S_ISCHR macro isn't in libc as well.
        let is_char_device = (mode & libc::S_IFMT) == libc::S_IFCHR;

        if is_char_device {
            unsafe { libc::isatty(fildes) != 0 }
        } else {
            false
        }
    } else {
        false
    }
}

#[cfg(test)]
mod test {
    use super::{os_string_from_ptr, string_from_ptr};

    #[test]
    fn miri_test_str_to_ptr() {
        let strp = |ptr| unsafe { string_from_ptr(ptr) };
        assert_eq!(strp(std::ptr::null()), "");
        assert_eq!(strp("\0".as_ptr() as *const libc::c_char), "");
        assert_eq!(strp("hello\0".as_ptr() as *const libc::c_char), "hello");
    }

    #[test]
    fn miri_test_os_str_to_ptr() {
        let strp = |ptr| unsafe { os_string_from_ptr(ptr) };
        assert_eq!(strp(std::ptr::null()), "");
        assert_eq!(strp("\0".as_ptr() as *const libc::c_char), "");
        assert_eq!(strp("hello\0".as_ptr() as *const libc::c_char), "hello");
    }

    #[test]
    fn test_tty() {
        use std::fs::File;
        use std::os::fd::AsRawFd;
        assert!(!super::safe_isatty(
            File::open("/bin/sh").unwrap().as_raw_fd()
        ));
        assert!(!super::safe_isatty(-837492));
        let (mut leader, mut follower) = Default::default();
        assert!(
            unsafe {
                libc::openpty(
                    &mut leader,
                    &mut follower,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            } == 0
        );
        assert!(super::safe_isatty(leader));
        assert!(super::safe_isatty(follower));
        unsafe {
            libc::close(follower);
            libc::close(leader);
        }
    }
}
