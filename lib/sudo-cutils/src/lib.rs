use std::{
    ffi::{CStr, OsStr, OsString},
    os::unix::prelude::OsStrExt,
};

pub fn cerr(res: libc::c_int) -> std::io::Result<libc::c_int> {
    match res {
        -1 => Err(std::io::Error::last_os_error()),
        _ => Ok(res),
    }
}

pub fn cerr_long(res: libc::c_long) -> std::io::Result<libc::c_long> {
    match res {
        -1 => Err(std::io::Error::last_os_error()),
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
    match cerr_long(unsafe { libc::sysconf(name) }) {
        Ok(res) => Some(res),
        Err(_) => None,
    }
}

/// Create a Rust string copy from a C string pointer
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

/// Create a C string copy of a Rust string copy, allocated by libc::malloc()
///
/// The returned pointer **must** be cleaned up via a call to `libc::free`.
pub fn into_leaky_cstring(s: &str) -> *const libc::c_char {
    let alloc_len: isize = s.len().try_into().expect("absurd string size");
    let mem = unsafe { libc::malloc(alloc_len as usize + 1) } as *mut u8;
    if mem.is_null() {
        panic!("libc malloc failed");
    } else {
        unsafe {
            std::ptr::copy_nonoverlapping(s.as_ptr(), mem, alloc_len as usize);
            *mem.offset(alloc_len) = 0;
        }
    }

    mem as *mut libc::c_char
}

/// A "secure" storage that gets wiped before dropping; inspired by Conrad Kleinespel's
/// Rustatic rtoolbox::SafeString, https://crates.io/crates/rtoolbox/0.0.1 and std::Pin<>
pub struct Secure<T: AsMut<[u8]>>(T);

impl<T: AsMut<[u8]>> Secure<T> {
    pub fn new(value: T) -> Self {
        Secure(value)
    }
}

impl<T: AsMut<[u8]>> std::ops::Deref for Secure<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: AsMut<[u8]>> Drop for Secure<T> {
    fn drop(&mut self) {
        wipe_memory(self.0.as_mut())
    }
}

/// Used to zero out memory and protect sensitive data from leaking; inspired by Conrad Kleinespel's
/// Rustatic rtoolbox::SafeString, https://crates.io/crates/rtoolbox/0.0.1
fn wipe_memory(memory: &mut [u8]) {
    use std::sync::atomic;

    let nonsense: u8 = 0x55;
    for c in memory {
        unsafe { std::ptr::write_volatile(c, nonsense) };
    }

    atomic::fence(atomic::Ordering::SeqCst);
    atomic::compiler_fence(atomic::Ordering::SeqCst);
}

#[cfg(test)]
mod test {
    use super::{into_leaky_cstring, os_string_from_ptr, string_from_ptr};

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
    fn miri_test_leaky_cstring() {
        let test = |text| unsafe {
            let ptr = into_leaky_cstring(text);
            let result = string_from_ptr(ptr);
            libc::free(ptr as *mut libc::c_void);
            result
        };
        assert_eq!(test(""), "");
        assert_eq!(test("hello"), "hello");
    }
}
