use std::{
    io::{self, Read, Write},
    marker::PhantomData,
    os::fd::AsRawFd,
};

use crate::{
    exec::event::{EventHandle, EventRegistry, Process},
    system::poll::PollEvent,
};

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
            PollEvent::Readable => self.buffer_lr.insert(&mut self.left, registry),
            PollEvent::Writable => self.buffer_rl.remove(&mut self.left, registry),
        }
    }

    /// Handle a poll event for the right side of the pipe.
    pub(super) fn on_right_event<T: Process>(
        &mut self,
        poll_event: PollEvent,
        registry: &mut EventRegistry<T>,
    ) -> io::Result<()> {
        match poll_event {
            PollEvent::Readable => self.buffer_rl.insert(&mut self.right, registry),
            PollEvent::Writable => self.buffer_lr.remove(&mut self.right, registry),
        }
    }

    /// Ensure that all the contents of the pipe's internal buffer are written to the left side.
    pub(super) fn flush_left(&mut self) -> io::Result<()> {
        self.buffer_rl.flush(&mut self.left)
    }
}

/// A circular buffer that stores the bytes read from `R` before they are written to `W`.
///
struct Buffer<R, W> {
    /// The internal buffer.
    /// ```text
    ///
    /// ┌───┬───┬───┬───┬───┬───┬───┬───┐
    /// │   │ M │ D │ D │ D │   │   │   │
    /// └───┴───┴───┴───┴───┴───┴───┴───┘
    ///           ▲           ▲
    ///           │           │
    ///         head         tail
    /// ```
    /// The extra byte (`M` in the diagram) is used as a marker to differentiate an empty buffer
    /// from a full one and it is always located one position before head.
    buffer: [u8; MAX_POS + 1],
    /// The first location of the buffer that has data.
    head: usize,
    /// The next location at which new data will be inserted.
    tail: usize,
    /// The handle for the event of the reader.
    read_handle: EventHandle,
    /// The handle for the event of the writer.
    write_handle: EventHandle,
    /// A type marker so we don't read or write from the wrong end.
    marker: PhantomData<(R, W)>,
}

/// The maximum value that head and tail can take.
const MAX_POS: usize = 6 * 1024 - 1;

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
            buffer: [0; MAX_POS + 1],
            head: 0,
            tail: 0,
            read_handle,
            write_handle,
            marker: PhantomData,
        }
    }

    /// Return true if the buffer is empty.
    fn is_empty(&self) -> bool {
        // The buffer is empty if head and tail overlap.
        // ┌───┬───┬───┬───┬───┬───┬───┬───┐
        // │   │   │ M │   │   │   │   │   │
        // └───┴───┴───┴───┴───┴───┴───┴───┘
        //               ▲
        //               │
        //         head and tail
        self.head == self.tail
    }

    /// Return true if the buffer is full.
    fn is_full(&self) -> bool {
        // The buffer is full if head is one position ahead of tail.
        // ┌───┬───┬───┬───┬───┬───┬───┬───┐
        // │ D │ D │ M │ D │ D │ D │ D │ D │
        // └───┴───┴───┴───┴───┴───┴───┴───┘
        //           ▲   ▲
        //           │   │
        //         tail head
        // Or if head is the first position and tail is the last position.
        // ┌───┬───┬───┬───┬───┬───┬───┬───┐
        // │ D │ D │ D │ D │ D │ D │ D │ M │
        // └───┴───┴───┴───┴───┴───┴───┴───┘
        //   ▲                           ▲
        //   │                           │
        // head                         tail
        // This can also be thought as tail pointing to the marker byte.
        self.head == (self.tail + 1) % self.buffer.len()
    }

    /// Read bytes and insert them into the buffer.
    ///
    /// Calling this function will block until `read` is ready to be read.
    fn insert<T: Process>(
        &mut self,
        read: &mut R,
        registry: &mut EventRegistry<T>,
    ) -> io::Result<()> {
        // Don't insert anything if the buffer is full.
        if self.is_full() {
            self.read_handle.ignore(registry);
            return Ok(());
        }

        // We need to find the empty sub-buffer that starts at tail. To do this, we must
        // ensure that we're not including the marker byte or any other byte after it.
        let buffer = if self.tail >= self.head {
            // If tail is greater than head, the sub-buffer extends from tail to the end of the
            // buffer.
            //                     ┌───────────┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │   │ M │ D │ D │ D │   │   │   │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //           ▲           ▲
            //           │           │
            //         head         tail
            // In the extreme case where tail is the last position, the sub-buffer will have a
            // length of one.
            //                             ┌───┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │   │ M │ D │ D │ D │ D │ D │   │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //           ▲                   ▲
            //           │                   │
            //         head                 tail
            // We can be sure that the marker byte is not at tail because the buffer is not full.
            // On the other hand if tail is equal to head, the buffer is empty
            //                     ┌───────────┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │   │   │   │   │ M │   │   │   │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //                       ▲
            //                       │
            //                 head and tail
            //
            &mut self.buffer[self.tail..]
        } else {
            // If tail is less than head, the empty sub-buffer extends from tail to the marker byte
            // without including the latter. We know that the marker byte is one position before
            // head.
            //     ┌───────────┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │ D │   │   │   │ M │ D │ D │ D │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //       ▲               ▲
            //       │               │
            //     tail             head
            //
            // In the extreme case where tail is as close as it can be to head, the sub-buffer will
            // have a length of one. We can be sure that the marker byte is not at tail because the
            // buffer is not full.
            //     ┌───┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │ D │   │ M │ D │ D │ D │ D │ D │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //       ▲       ▲
            //       │       │
            //     tail     head
            &mut self.buffer[self.tail..(self.head - 1)]
        };

        // Insert `len` bytes from `read` into the buffer.
        let len = read.read(buffer)?;

        if len > 0 {
            // We update tail so it becomes the next position where we can insert data again.
            self.tail += len;
            // However, we must be sure that tail doesn't go past the last position.
            self.tail %= self.buffer.len();
            // If we actually inserted something, the buffer is not empty anymore and we can resume
            // writing.
            self.write_handle.resume(registry);
        }

        Ok(())
    }

    /// Remove bytes from the buffer and write them.
    ///
    /// Calling this function will block until `write` is ready to be written.
    fn remove<T: Process>(
        &mut self,
        write: &mut W,
        registry: &mut EventRegistry<T>,
    ) -> io::Result<()> {
        // Don't remove anything if the buffer is empty.
        if self.is_empty() {
            self.write_handle.ignore(registry);
            return Ok(());
        }

        // We need to find the sub-buffer with data that starts at head. To do this, we must ensure
        // that we're not including the end byte or any other empty byte after it.
        let buffer = if self.head < self.tail {
            // If head is less than tail, the sub-buffer extends from head to one position before
            // tail.
            //         ┌───────────┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │   │ M │ D │ D │ D │   │   │   │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //           ▲           ▲
            //           │           │
            //         head         tail
            // In the extreme case where head is as close as possible to tail, the sub-buffer will have a
            // length of one. We can be sure that head and tail are not equal because the buffer is
            // not empty.
            //         ┌───┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │   │ M │ D │   │   │   │   │   │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //           ▲   ▲
            //           │   │
            //         head  tail
            &mut self.buffer[self.head..self.tail]
        } else {
            // If head is greater than tail, the sub-buffer extends from head to the end of the
            // buffer.
            //                     ┌───────────┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │ D │   │   │   │ M │ D │ D │ D │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //       ▲               ▲
            //       │               │
            //     tail             head
            //
            // In the extreme case where head is the last position, the sub-buffer will have a
            // length of one.
            //                             ┌───┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │ D │   │   │   │   │   │ M │ D │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //       ▲                       ▲
            //       │                       │
            //     tail                     head
            // On the other hand if tail is as close as it can be to head, the buffer is full
            //             ┌───────────────────┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │ D │ D │ M │ D │ D │ D │ D │ D │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //           ▲   ▲
            //           │   │
            //         tail head
            // We know they cannot be equal because the buffer is not empty.
            &mut self.buffer[self.head..]
        };

        // Remove `len` bytes from the buffer and write them.
        let len = write.write(buffer)?;

        if len > 0 {
            // We update head so it becomes the first position with data again.
            self.head += len;
            // However, we must be sure that tail doesn't go past the last position.
            self.head %= self.buffer.len();
            // If we actually removed something, the buffer is not full anymore and we can resume
            // reading.
            self.read_handle.resume(registry);
        }

        Ok(())
    }

    /// Flush this buffer, ensuring that all the contents of its internal buffer are written.
    fn flush(&mut self, write: &mut W) -> io::Result<()> {
        // Don't remove anything if the buffer is empty.
        if self.is_empty() {
            return write.flush();
        }

        // We need to find all the sub-buffers with data.
        if self.head < self.tail {
            // If head is less than tail, there is a single sub-buffer with data that extends from
            // head to one position before tail.
            //         ┌───────────┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │   │ M │ D │ D │ D │   │   │   │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //           ▲           ▲
            //           │           │
            //         head         tail
            // In the extreme case where head is as close as possible to tail, the sub-buffer will have a
            // length of one. We can be sure that head and tail are not equal because the buffer is
            // not empty.
            //         ┌───┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │   │ M │ D │   │   │   │   │   │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //           ▲   ▲
            //           │   │
            //         head  tail
            write.write_all(&self.buffer[self.head..self.tail])?;
        } else {
            // If head is greater than tail, there are two sub-buffers with data. The first one
            // extends from head to the end of the buffer. and the second one goes from the start
            // of the buffer to one position before tail.
            // ┌───┐               ┌───────────┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │ D │   │   │   │ M │ D │ D │ D │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //       ▲               ▲
            //       │               │
            //     tail             head
            //
            // In the extreme case where head is the last position, the first sub-buffer will have
            // a length of one.
            // ┌───────┐                   ┌───┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │ D │ D │   │   │   │   │ M │ D │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //           ▲                   ▲
            //           │                   │
            //         tail                 head
            // On the other hand if tail is as close as it can be to head, the buffer is full
            // ┌───────┐   ┌───────────────────┐
            // ┌───┬───┬───┬───┬───┬───┬───┬───┐
            // │ D │ D │ M │ D │ D │ D │ D │ D │
            // └───┴───┴───┴───┴───┴───┴───┴───┘
            //           ▲   ▲
            //           │   │
            //         tail head
            // We know they cannot be equal because the buffer is not empty.
            write.write_all(&self.buffer[self.head..])?;
            write.write_all(&self.buffer[..self.tail])?;
        };

        // Now that we have written all the data from the buffer. We can mark it as empty
        self.head = 0;
        self.tail = 0;

        write.flush()
    }
}
