use std::io::{self, IoSlice, IoSliceMut, Read, Write};

pub(super) struct RingBuffer {
    storage: Box<[u8; Self::LEN]>,
    // The start index of the non-empty section of the buffer.
    start: usize,
    // The length of the non-empty section of the buffer.
    len: usize,
}

impl RingBuffer {
    /// The size of the internal storage of the ring buffer.
    const LEN: usize = 8 * 1024;

    /// Create a new, empty buffer.
    pub(super) fn new() -> Self {
        Self {
            storage: Box::new([0; Self::LEN]),
            start: 0,
            len: 0,
        }
    }

    pub(super) fn is_full(&self) -> bool {
        self.len == self.storage.len()
    }

    // rustc 1.77.1 clippy gives false diagnostics, https://github.com/rust-lang/rust-clippy/issues/12519
    #[allow(clippy::unused_io_amount)]
    pub(super) fn insert<R: Read>(&mut self, read: &mut R) -> io::Result<usize> {
        let inserted_len = if self.is_empty() {
            // Case 1.1. The buffer is empty, meaning that there are two unfilled slices in
            // `storage`:`start..` and `..start`.
            let (second_slice, first_slice) = self.storage.split_at_mut(self.start);
            read.read_vectored(&mut [first_slice, second_slice].map(IoSliceMut::new))?
        } else {
            let &mut Self { start, len, .. } = self;
            let end = start + len;
            if end >= self.storage.len() {
                // Case 1.2. The buffer is not empty and the filled section wraps around `storage`.
                // Meaning that there is only one unfilled slice in `storage`: `end..start`.
                let end = end % self.storage.len();
                read.read(&mut self.storage[end..start])?
            } else {
                // Case 1.3. The buffer is not empty and the filled section is a contiguous slice
                // of `storage`. Meaning that there are two unfilled slices in `storage`: `..start`
                // and `end..`.
                let (mid, first_slice) = self.storage.split_at_mut(end);
                let second_slice = &mut mid[..start];
                read.read_vectored(&mut [first_slice, second_slice].map(IoSliceMut::new))?
            }
        };

        self.len += inserted_len;

        debug_assert!(self.start < Self::LEN);
        debug_assert!(self.len <= Self::LEN);

        Ok(inserted_len)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }

    // rustc 1.77.1 clippy gives false diagnostics, https://github.com/rust-lang/rust-clippy/issues/12519
    #[allow(clippy::unused_io_amount)]
    pub(super) fn remove<W: Write>(&mut self, write: &mut W) -> io::Result<usize> {
        let removed_len = if self.is_full() {
            // Case 2.1. The buffer is full, meaning that there are two filled slices in `storage`:
            // `start..` and `..start`.
            let (second_slice, first_slice) = self.storage.split_at(self.start);
            write.write_vectored(&[first_slice, second_slice].map(IoSlice::new))?
        } else {
            let end = self.start + self.len;
            if end >= self.storage.len() {
                // Case 2.2. The buffer is not full and the filled section wraps around `storage`.
                // Meaning that there are two non-empty slices in `storage`: `start..` and `..end`.
                let end = end % self.storage.len();
                let first_slice = &self.storage[self.start..];
                let second_slice = &self.storage[..end];
                write.write_vectored(&[first_slice, second_slice].map(IoSlice::new))?
            } else {
                // Case 2.3. The buffer is not full and the filled section is a contiguous slice
                // of `storage.` Meaning that there is only one filled slice in `storage`:
                // `start..end`.
                write.write(&self.storage[self.start..end])?
            }
        };

        self.start += removed_len;
        self.start %= Self::LEN;

        self.len -= removed_len;

        debug_assert!(self.start < Self::LEN);
        debug_assert!(self.len <= Self::LEN);

        Ok(removed_len)
    }
}

#[cfg(test)]
mod tests {
    use super::RingBuffer;

    #[test]
    fn empty_buffer_is_empty() {
        let buf = RingBuffer::new();

        assert!(buf.is_empty());
    }

    #[test]
    fn full_buffer_is_full() {
        let mut buf = RingBuffer::new();

        let inserted_len = buf.insert(&mut [0x45; RingBuffer::LEN].as_slice()).unwrap();
        assert_eq!(inserted_len, RingBuffer::LEN);

        assert!(buf.is_full());
    }

    #[test]
    fn buffer_is_fifo() {
        let mut buf = RingBuffer::new();

        let expected = (0..=u8::MAX).collect::<Vec<u8>>();
        let inserted_len = buf.insert(&mut expected.as_slice()).unwrap();
        assert_eq!(inserted_len, expected.len());

        let mut found = vec![];
        let removed_len = buf.remove(&mut found).unwrap();
        assert_eq!(removed_len, expected.len());

        assert_eq!(expected, found);
    }

    #[test]
    fn insert_into_empty_buffer_with_offset() {
        const HALF_LEN: usize = RingBuffer::LEN / 2;
        let mut buf = RingBuffer::new();

        // This should leave the buffer empty but with the start field pointing to the middle of
        // the buffer.
        // ┌───────────────────┐
        // │                   │
        // └───────────────────┘
        //           ▲
        //           │
        //         start
        buf.insert(&mut [0u8; HALF_LEN].as_slice()).unwrap();
        buf.remove(&mut vec![]).unwrap();

        assert_eq!(buf.start, HALF_LEN);
        assert_eq!(buf.len, 0);

        // Then we fill the first half of the buffer with ones and the second one with twos in a
        // single insertion. This tests case 1.1.
        // ┌─────────┬─────────┐
        // │    2    │    1    │
        // └─────────┴─────────┘
        //           ▲
        //           │
        //         start
        let mut expected = vec![1; HALF_LEN];
        expected.extend_from_slice(&[2; HALF_LEN]);
        buf.insert(&mut expected.as_slice()).unwrap();

        assert_eq!(buf.start, HALF_LEN);
        assert_eq!(buf.len, RingBuffer::LEN);

        // When we remove all the elements of the buffer we should find them in the same order we
        // inserted them. This tests case 2.1.
        let mut found = vec![];
        let removed_len = buf.remove(&mut found).unwrap();
        assert_eq!(removed_len, expected.len());

        assert_eq!(expected, found);

        assert_eq!(buf.start, HALF_LEN);
        assert_eq!(buf.len, 0);
    }

    #[test]
    fn insert_into_non_empty_wrapping_buffer() {
        const QUARTER_LEN: usize = RingBuffer::LEN / 4;
        let mut buf = RingBuffer::new();

        // This should leave the buffer empty but with the start field pointing to the middle of
        // the buffer.
        // ┌───────────────────────┐
        // │                       │
        // └───────────────────────┘
        //             ▲
        //             │
        //           start
        buf.insert(&mut [0; 2 * QUARTER_LEN].as_slice()).unwrap();
        buf.remove(&mut vec![]).unwrap();

        assert_eq!(buf.start, 2 * QUARTER_LEN);
        assert_eq!(buf.len, 0);

        // Then we fill one quarter of the buffer with ones. This gives us a non-empty buffer whose
        // empty section is not contiguous.
        // ┌───────────┬─────┬─────┐
        // │           │  1  │     │
        // └───────────┴─────┴─────┘
        //             ▲
        //             │
        //           start
        let mut expected = vec![1; QUARTER_LEN];
        buf.insert(&mut expected.as_slice()).unwrap();

        assert_eq!(buf.start, 2 * QUARTER_LEN);
        assert_eq!(buf.len, QUARTER_LEN);

        // Then we fill one quarter of the buffer with twos and another quarter of the buffer with
        // threes in a single insertion. This tests case 1.2.
        // ┌─────┬─────┬─────┬─────┐
        // │  3  │     │  1  │  2  │
        // └─────┴─────┴─────┴─────┘
        //             ▲
        //             │
        //           start
        let mut second_half = vec![2; QUARTER_LEN];
        second_half.extend_from_slice(&[3; QUARTER_LEN]);
        buf.insert(&mut second_half.as_slice()).unwrap();

        expected.extend(second_half);

        assert_eq!(buf.start, 2 * QUARTER_LEN);
        assert_eq!(buf.len, 3 * QUARTER_LEN);

        // When we remove all the elements of the buffer we should find them in the same order we
        // inserted them. This tests case 2.2.
        let mut found = vec![];
        let removed_len = buf.remove(&mut found).unwrap();
        assert_eq!(removed_len, expected.len());

        assert_eq!(expected, found);

        assert_eq!(buf.start, QUARTER_LEN);
        assert_eq!(buf.len, 0);
    }

    #[test]
    fn insert_into_non_empty_non_wrapping_buffer() {
        const QUARTER_LEN: usize = RingBuffer::LEN / 4;
        let mut buf = RingBuffer::new();

        // We fill one quarter of the buffer with ones. This gives us a non-empty buffer whose
        // empty section is contiguous.
        // ┌─────┬────────────────┐
        // │  1  │                │
        // └─────┴────────────────┘
        // ▲
        // │
        // └ start
        let mut expected = vec![1; QUARTER_LEN];
        buf.insert(&mut expected.as_slice()).unwrap();

        assert_eq!(buf.start, 0);
        assert_eq!(buf.len, QUARTER_LEN);

        // Then we fill one quarter of the buffer with twos. This tests case 1.3.
        // ┌─────┬─────┬──────────┐
        // │  1  │  2  │          │
        // └─────┴─────┴──────────┘
        // ▲
        // │
        // └ start
        let second_half = vec![2; QUARTER_LEN];
        buf.insert(&mut second_half.as_slice()).unwrap();

        expected.extend(second_half);

        assert_eq!(buf.start, 0);
        assert_eq!(buf.len, 2 * QUARTER_LEN);

        // When we remove all the elements of the buffer we should find them in the same order we
        // inserted them. This tests case 2.3.
        let mut found = vec![];
        let removed_len = buf.remove(&mut found).unwrap();
        assert_eq!(removed_len, expected.len());

        assert_eq!(expected, found);

        assert_eq!(buf.start, 2 * QUARTER_LEN);
        assert_eq!(buf.len, 0);
    }
}
