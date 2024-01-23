use std::{
    io::{self, IoSlice, IoSliceMut, Read, Write},
    marker::PhantomData,
    os::fd::AsRawFd,
};

use crate::exec::event::{EventHandle, EventRegistry, PollEvent, Process};

// A pipe able to stream data bidirectionally between two read-write types.
pub(super) struct Pipe<L, R> {
    left: L,
    right: R,
    buffer_lr: Buffer<L, R>,
    buffer_rl: Buffer<R, L>,
}

impl<L: Read + Write + AsRawFd, R: Read + Write + AsRawFd> Pipe<L, R> {
    /// Create a new pipe between two read-write types and register them to be polled.
    pub fn new<T: Process>(
        left: L,
        right: R,
        registry: &mut EventRegistry<T>,
        f_left: fn(PollEvent) -> T::Event,
        f_right: fn(PollEvent) -> T::Event,
    ) -> Self {
        Self {
            buffer_lr: Buffer::new(
                registry.register_event(&left, PollEvent::Readable, f_left),
                registry.register_event(&right, PollEvent::Writable, f_right),
                registry,
            ),
            buffer_rl: Buffer::new(
                registry.register_event(&right, PollEvent::Readable, f_right),
                registry.register_event(&left, PollEvent::Writable, f_left),
                registry,
            ),
            left,
            right,
        }
    }

    /// Get a reference to the left side of the pipe.
    pub(super) fn left(&self) -> &L {
        &self.left
    }

    /// Get a mutable reference to the left side of the pipe.
    pub(super) fn left_mut(&mut self) -> &mut L {
        &mut self.left
    }

    /// Get a reference to the right side of the pipe.
    pub(super) fn right(&self) -> &R {
        &self.right
    }

    /// Stop the poll events of this pipe.
    pub(super) fn ignore_events<T: Process>(&mut self, registry: &mut EventRegistry<T>) {
        self.buffer_lr.read_handle.ignore(registry);
        self.buffer_lr.write_handle.ignore(registry);
        self.buffer_rl.read_handle.ignore(registry);
        self.buffer_rl.write_handle.ignore(registry);
    }

    /// Resume the poll events of this pipe
    pub(super) fn resume_events<T: Process>(&mut self, registry: &mut EventRegistry<T>) {
        self.buffer_lr.read_handle.resume(registry);
        self.buffer_lr.write_handle.resume(registry);
        self.buffer_rl.read_handle.resume(registry);
        self.buffer_rl.write_handle.resume(registry);
    }

    /// Handle a poll event for the left side of the pipe.
    pub(super) fn on_left_event<T: Process>(
        &mut self,
        poll_event: PollEvent,
        registry: &mut EventRegistry<T>,
    ) -> io::Result<()> {
        match poll_event {
            PollEvent::Readable => self.buffer_lr.read(&mut self.left, registry),
            PollEvent::Writable => self.buffer_rl.write(&mut self.left, registry),
        }
    }

    /// Handle a poll event for the right side of the pipe.
    pub(super) fn on_right_event<T: Process>(
        &mut self,
        poll_event: PollEvent,
        registry: &mut EventRegistry<T>,
    ) -> io::Result<()> {
        match poll_event {
            PollEvent::Readable => self.buffer_rl.read(&mut self.right, registry),
            PollEvent::Writable => self.buffer_lr.write(&mut self.right, registry),
        }
    }

    /// Ensure that all the contents of the pipe's internal buffer are written to the left side.
    pub(super) fn flush_left(&mut self) -> io::Result<()> {
        self.buffer_rl.flush(&mut self.left)
    }
}

struct RingBuffer {
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
    fn new() -> Self {
        Self {
            storage: Box::new([0; Self::LEN]),
            start: 0,
            len: 0,
        }
    }

    fn is_full(&self) -> bool {
        self.len == self.storage.len()
    }

    fn insert<R: Read>(&mut self, read: &mut R) -> io::Result<usize> {
        let inserted_len = if self.is_empty() {
            // Case 1.1. The buffer is empty, meaning that there are two empty slices in `storage`:
            // `start..` and `..start`.
            let (second_slice, first_slice) = self.storage.split_at_mut(self.start);
            read.read_vectored(&mut [first_slice, second_slice].map(IoSliceMut::new))?
        } else {
            let &mut Self { start, len, .. } = self;
            let end = start + len;
            if end >= self.storage.len() {
                // Case 1.2. The buffer is not empty and the non-empty section wraps around
                // `storage`. Meaning that there is only one empty slice in `storage`: `end..start`.
                let end = end % self.storage.len();
                read.read(&mut self.storage[end..start])?
            } else {
                // Case 1.3. The buffer is non empty and the non-empty section is a contiguous
                // slice of `storage`. Meaning that there are two empty slices in `storage`:
                // `..start` and `end..`.
                let (mid, first_slice) = self.storage.split_at_mut(end);
                let second_slice = &mut mid[..start];
                read.read_vectored(&mut [first_slice, second_slice].map(IoSliceMut::new))?
            }
        };

        self.len += inserted_len;

        Ok(inserted_len)
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn remove<W: Write>(&mut self, write: &mut W) -> io::Result<usize> {
        let removed_len = if self.is_full() {
            // Case 2.1. The buffer is full, meaning that there are two non-empty slices in
            // `storage`: `start..` and `..start`.
            let (second_slice, first_slice) = self.storage.split_at(self.start);
            write.write_vectored(&[first_slice, second_slice].map(IoSlice::new))?
        } else {
            let end = self.start + self.len;
            if end >= self.storage.len() {
                // Case 2.2. The buffer is not full and the non-empty section wraps around
                // `storage`. Meanning that there are two non-empty slices in `storage`: `start..`
                // and `..end`.
                let end = end % self.storage.len();
                let first_slice = &self.storage[self.start..];
                let second_slice = &self.storage[..end];
                write.write_vectored(&[first_slice, second_slice].map(IoSlice::new))?
            } else {
                // Case 2.3. The buffer is not full and the non-empty section is a contiguous slice
                // of `storage.` Meaning that there is only one non-empty slice in `storage`:
                // `start..end`.
                write.write(&self.storage[self.start..end])?
            }
        };

        self.start += removed_len;
        self.start %= self.storage.len();

        self.len -= removed_len;

        Ok(removed_len)
    }
}

/// A buffer that stores the bytes read from `R` before they are written to `W`.
struct Buffer<R, W> {
    internal: RingBuffer,
    /// The handle for the event of the reader.
    read_handle: EventHandle,
    /// The handle for the event of the writer.
    write_handle: EventHandle,
    marker: PhantomData<(R, W)>,
}

impl<R: Read, W: Write> Buffer<R, W> {
    /// Create a new, empty buffer
    fn new<T: Process>(
        read_handle: EventHandle,
        mut write_handle: EventHandle,
        registry: &mut EventRegistry<T>,
    ) -> Self {
        // The buffer is empty, don't write
        write_handle.ignore(registry);

        Self {
            internal: RingBuffer::new(),
            read_handle,
            write_handle,
            marker: PhantomData,
        }
    }

    /// Read bytes into the buffer.
    ///
    /// Calling this function will block until `read` is ready to be read.
    fn read<T: Process>(
        &mut self,
        read: &mut R,
        registry: &mut EventRegistry<T>,
    ) -> io::Result<()> {
        // If the buffer is full, there is nothing to be read.
        if self.internal.is_full() {
            self.read_handle.ignore(registry);
            return Ok(());
        }

        // Read bytes and insert them into the buffer.
        let inserted_len = self.internal.insert(read)?;

        // If we inserted something, the buffer is not empty anymore and we can resume writing.
        if inserted_len > 0 {
            self.write_handle.resume(registry);
        }

        Ok(())
    }

    /// Write bytes from the buffer.
    ///
    /// Calling this function will block until `write` is ready to be written.
    fn write<T: Process>(
        &mut self,
        write: &mut W,
        registry: &mut EventRegistry<T>,
    ) -> io::Result<()> {
        // If the buffer is empty, there is nothing to be written.
        if self.internal.is_empty() {
            self.write_handle.ignore(registry);
            return Ok(());
        }

        // Remove bytes from the buffer and write them.
        let removed_len = self.internal.remove(write)?;

        // If we removed something, the buffer is not full anymore and we can resume reading.
        if removed_len > 0 {
            self.read_handle.resume(registry);
        }

        Ok(())
    }

    /// Flush this buffer, ensuring that all the contents of its internal buffer are written.
    fn flush(&mut self, write: &mut W) -> io::Result<()> {
        // Remove bytes from the buffer and write them.
        self.internal.remove(write)?;

        write.flush()
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

        // When we remove all the elements of the buffer we should find them in the same order we
        // inserted them. This tests case 2.1.
        let mut found = vec![];
        let removed_len = buf.remove(&mut found).unwrap();
        assert_eq!(removed_len, expected.len());

        assert_eq!(expected, found);
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

        // When we remove all the elements of the buffer we should find them in the same order we
        // inserted them. This tests case 2.2.
        let mut found = vec![];
        let removed_len = buf.remove(&mut found).unwrap();
        assert_eq!(removed_len, expected.len());

        assert_eq!(expected, found);
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

        // When we remove all the elements of the buffer we should find them in the same order we
        // inserted them. This tests case 2.3.
        let mut found = vec![];
        let removed_len = buf.remove(&mut found).unwrap();
        assert_eq!(removed_len, expected.len());

        assert_eq!(expected, found);
    }
}
