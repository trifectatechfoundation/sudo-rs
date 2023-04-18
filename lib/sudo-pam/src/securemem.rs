///! Routines for "secure" memory operations; i.e. data that we need to send to Linux-PAM and don't
///! want any copies to leak (that we would then need to zeroize).
use std::slice;

pub struct PamBuffer(*mut u8);

impl PamBuffer {
    const SIZE: usize = sudo_pam_sys::PAM_MAX_RESP_SIZE as usize;

    // consume this buffer and return its internal pointer
    // (ending the type-level security, but guaranteeing you need unsafe code to access the data)
    pub fn leak(self) -> *const u8 {
        let result = self.0;
        std::mem::forget(self);

        result
    }

    // initialize the buffer with already existing data (otherwise populating it is a bit hairy)
    // this is inferior than placing the data into the securebuffer directly
    #[cfg(test)]
    pub fn new(mut src: impl AsMut<[u8]>) -> Self {
        let mut buffer = PamBuffer::default();
        let src = src.as_mut();
        buffer[..src.len()].copy_from_slice(src);
        wipe_memory(src);

        buffer
    }
}

impl Default for PamBuffer {
    fn default() -> Self {
        PamBuffer(unsafe { libc::calloc(1, Self::SIZE) as *mut u8 })
    }
}

impl std::ops::Deref for PamBuffer {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        // make the slice one less in size to guarantee the existence of a terminating NUL
        unsafe { slice::from_raw_parts(self.0, Self::SIZE - 1) }
    }
}

impl std::ops::DerefMut for PamBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.0, Self::SIZE - 1) }
    }
}

impl Drop for PamBuffer {
    fn drop(&mut self) {
        if !self.0.is_null() {
            wipe_memory(unsafe { &mut *(self.0 as *mut [u8; Self::SIZE]) });
            unsafe { libc::free(self.0 as *mut _) }
        }
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
    use super::PamBuffer;

    #[test]
    fn miri_test_leaky_cstring() {
        let test = |text: &str| unsafe {
            let buf = PamBuffer::new(text.to_string().as_bytes_mut());
            assert_eq!(&buf[..text.len()], text.as_bytes());
            let ptr = buf.leak();
            let result = sudo_cutils::string_from_ptr(ptr as *mut _);
            libc::free(ptr as *mut libc::c_void);
            result
        };
        assert_eq!(test(""), "");
        assert_eq!(test("hello"), "hello");
    }

    #[test]
    fn miri_test_wipe() {
        let mut memory: [u8; 3] = [1, 2, 3];
        let fix = PamBuffer::new(&mut memory);
        assert_eq!(memory, [0x55, 0x55, 0x55]);
        assert_eq!(fix[0..=2], [1, 2, 3]);
        assert!(fix[3..].iter().all(|&x| x == 0));
        std::mem::drop(fix);
    }
}
