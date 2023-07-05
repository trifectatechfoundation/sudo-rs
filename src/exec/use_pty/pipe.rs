use std::{
    io::{self, Read, Write},
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

/// The size of the internal buffer of the pipe.
const BUFSIZE: usize = 6 * 1024;

/// A buffer that stores the bytes read from `R` before they are written to `W`.
struct Buffer<R, W> {
    buffer: [u8; BUFSIZE],
    /// The start of the busy section of the buffer.
    start: usize,
    /// The end of the busy section of the buffer.
    end: usize,
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
            buffer: [0; BUFSIZE],
            start: 0,
            end: 0,
            read_handle,
            write_handle,
            marker: PhantomData,
        }
    }

    /// Return true if the buffer is empty.
    fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Return true if the buffer is full.
    fn is_full(&self) -> bool {
        // FIXME: This doesn't really mean that the buffer is full but it cannot be used for writes
        // anyway.
        self.end == BUFSIZE
    }

    /// Read bytes into the buffer.
    ///
    /// Calling this function will block until `read` is ready to be read.
    fn read<T: Process>(
        &mut self,
        read: &mut R,
        registry: &mut EventRegistry<T>,
    ) -> io::Result<()> {
        // Don't read if the buffer is full.
        if self.is_full() {
            self.read_handle.ignore(registry);
            return Ok(());
        }

        // This is the remaining free section that follows the busy section of the buffer.
        let buffer = &mut self.buffer[self.end..];

        // Read `len` bytes from `read` into the buffer.
        let len = read.read(buffer)?;

        // Mark the `len` bytes after the busy section as busy too.
        self.end += len;

        // If we read something, the buffer is not empty anymore and we can resume writing.
        if len > 0 {
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
        // Don't write if the buffer is empty.
        if self.is_empty() {
            self.write_handle.ignore(registry);
            return Ok(());
        }

        // This is the busy section of the buffer.
        let buffer = &self.buffer[self.start..self.end];

        // Write the first `len` bytes of the busy section to `write`.
        let len = write.write(buffer)?;

        if len == buffer.len() {
            // If we were able to write all the busy section, we can mark the whole buffer as free.
            self.start = 0;
            self.end = 0;
        } else {
            // Otherwise we just free the first `len` bytes of the busy section.
            self.start += len;
        }

        // If we wrote something, the buffer is not full anymore and we can resume reading.
        if len > 0 {
            self.read_handle.resume(registry);
        }

        Ok(())
    }

    /// Flush this buffer, ensuring that all the contents of its internal buffer are written.
    fn flush(&mut self, write: &mut W) -> io::Result<()> {
        // This is the busy section of the buffer.
        let buffer = &self.buffer[self.start..self.end];

        // Write the complete busy section to `write`.
        write.write_all(buffer)?;

        // If we were able to write all the busy section, we can mark the whole buffer as free.
        self.start = 0;
        self.end = 0;

        write.flush()
    }
}
