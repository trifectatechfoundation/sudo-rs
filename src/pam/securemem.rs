//! Routines for "secure" memory operations; i.e. data that we need to send to Linux-PAM and don't
//! want any copies to leak (that we would then need to zeroize).
use std::{
    alloc::{self, Layout},
    ptr::NonNull,
    slice,
};

const SIZE: usize = super::sys::PAM_MAX_RESP_SIZE as usize;

pub struct PamBuffer(NonNull<[u8; SIZE]>);

const LAYOUT: Layout = match Layout::from_size_align(SIZE, 1) {
    Ok(layout) => layout,
    Err(_) => unreachable!(),
};

impl PamBuffer {
    // consume this buffer and return its internal pointer
    // (ending the type-level security, but guaranteeing you need unsafe code to access the data)
    pub fn leak(self) -> NonNull<u8> {
        let result = self.0;
        std::mem::forget(self);

        result.cast()
    }

    // initialize the buffer with already existing data (otherwise populating it is a bit hairy)
    // this is inferior to placing the data into the securebuffer directly
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
        // SAFETY: `calloc` returns either a cleared, allocated chunk of `SIZE` bytes
        // or NULL to indicate that the allocation request failed
        let res = unsafe { libc::calloc(1, SIZE) };
        if let Some(nn) = NonNull::new(res) {
            PamBuffer(nn.cast())
        } else {
            alloc::handle_alloc_error(LAYOUT)
        }
    }
}

impl std::ops::Deref for PamBuffer {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        // SAFETY: `self.0.as_ptr()` is non-null, aligned, and initialized, and points to `SIZE` bytes.
        // The lifetime of the slice does not exceed that of `self`.
        //
        // We make the slice one less in size to guarantee the existence of a terminating NUL.
        unsafe { slice::from_raw_parts(self.0.as_ptr().cast(), SIZE - 1) }
    }
}

impl std::ops::DerefMut for PamBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        // SAFETY: see above
        unsafe { slice::from_raw_parts_mut(self.0.as_ptr().cast(), SIZE - 1) }
    }
}

impl Drop for PamBuffer {
    fn drop(&mut self) {
        // SAFETY: same as for `deref()` and `deref_mut()`
        wipe_memory(unsafe { self.0.as_mut() });
        // SAFETY: `self.0.as_ptr()` was obtained via `calloc`, so calling `free` is proper.
        unsafe { libc::free(self.0.as_ptr().cast()) }
    }
}

/// Used to zero out memory and protect sensitive data from leaking; inspired by Conrad Kleinespel's
/// Rustatic rtoolbox::SafeString, <https://crates.io/crates/rtoolbox/0.0.1>
fn wipe_memory(memory: &mut [u8]) {
    use std::sync::atomic;

    let nonsense: u8 = 0x55;
    for c in memory {
        // SAFETY: `c` is safe for writes (it comes from a &mut reference)
        unsafe { std::ptr::write_volatile(c, nonsense) };
    }

    atomic::fence(atomic::Ordering::SeqCst);
    atomic::compiler_fence(atomic::Ordering::SeqCst);
}

#[allow(clippy::undocumented_unsafe_blocks)]
#[cfg(test)]
mod test {
    use super::PamBuffer;

    #[test]
    fn miri_test_leaky_cstring() {
        let test = |text: &str| unsafe {
            let buf = PamBuffer::new(text.to_string().as_bytes_mut());
            assert_eq!(&buf[..text.len()], text.as_bytes());
            let nn = buf.leak();
            let result = crate::cutils::string_from_ptr(nn.as_ptr().cast());
            libc::free(nn.as_ptr().cast());
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
