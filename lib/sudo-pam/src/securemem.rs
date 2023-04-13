///! Routines for "secure" memory operations
pub type SecureBuffer = Secure<Vec<u8>>;

pub struct Secure<T: AsMut<[u8]>>(T);

impl<T: AsMut<[u8]> + AsRef<[u8]>> Secure<T> {
    pub fn new(value: T) -> Self {
        Secure(value)
    }

    pub fn leak(self) -> *const u8 {
        copy_as_libc_cstring(self.0.as_ref()) as *const _
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

/// Create a copy of a Rust byte slice as a null-terminated char pointer
/// (i.e. "a null terminated string") allocated by libc::malloc().
///
/// The returned pointer **must** be cleaned up via a call to `libc::free`.
fn copy_as_libc_cstring(s: &[u8]) -> *const libc::c_char {
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

#[cfg(test)]
mod test {
    #[test]
    fn miri_test_leaky_cstring() {
        let test = |text: &str| unsafe {
            let ptr = super::copy_as_libc_cstring(text.as_bytes());
            let result = sudo_cutils::string_from_ptr(ptr);
            libc::free(ptr as *mut libc::c_void);
            result
        };
        assert_eq!(test(""), "");
        assert_eq!(test("hello"), "hello");
    }

    #[test]
    fn miri_test_wipe() {
        let mut memory: [u8; 3] = [1, 2, 3];
        let fix = super::Secure::new(&mut memory);
        assert_eq!(*fix, &[1, 2, 3]);
        std::mem::drop(fix);
        assert_eq!(memory, [0x55, 0x55, 0x55]);
    }
}
