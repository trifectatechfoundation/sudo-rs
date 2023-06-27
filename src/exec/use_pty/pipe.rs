use std::{
    io::{self, Read, Write},
    marker::PhantomData,
    os::fd::AsRawFd,
};

use crate::{
    exec::event::{EventId, EventRegistry, Process},
    system::poll::PollEvent,
};

// A pipe able to stream data bidirectionally between two read-write types.
pub(super) struct Pipe<L, R> {
    left: L,
    right: R,
    buffer_lr: Buffer<L, R>,
    buffer_rl: Buffer<R, L>,
    left_ids: Option<(EventId, EventId)>,
    right_ids: Option<(EventId, EventId)>,
}

impl<L: Read + Write + AsRawFd, R: Read + Write + AsRawFd> Pipe<L, R> {
    /// Create a new pipe between two read-write types.
    pub fn new(left: L, right: R) -> Self {
        Self {
            left,
            right,
            buffer_lr: Buffer::new(),
            buffer_rl: Buffer::new(),
            left_ids: None,
            right_ids: None,
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

    /// Register the poll events of this pipe if they have not been registered yet.
    pub(super) fn register_events<T: Process>(
        &mut self,
        registry: &mut EventRegistry<T>,
        f_left: fn(PollEvent) -> T::Event,
        f_right: fn(PollEvent) -> T::Event,
    ) {
        if self.left_ids.is_none() {
            self.left_ids = Some(registry.register_rw_event(&self.left, f_left));
        }

        if self.right_ids.is_none() {
            self.right_ids = Some(registry.register_rw_event(&self.right, f_right));
        }
    }

    /// Deregister the poll events of this pipe if they have not been deregistered yet.
    pub(super) fn deregister_events<T: Process>(&mut self, registry: &mut EventRegistry<T>) {
        if let Some((read_id, write_id)) = self.left_ids.take() {
            registry.deregister_event(read_id);
            registry.deregister_event(write_id);
        }

        if let Some((read_id, write_id)) = self.right_ids.take() {
            registry.deregister_event(read_id);
            registry.deregister_event(write_id);
        }
    }

    /// Handle a poll event for the left side of the pipe.
    pub(super) fn on_left_event(&mut self, poll_event: PollEvent) -> io::Result<()> {
        match poll_event {
            PollEvent::Readable => self.buffer_lr.read(&mut self.left),
            PollEvent::Writable => self.buffer_rl.write(&mut self.left),
        }
    }

    /// Handle a poll event for the right side of the pipe.
    pub(super) fn on_right_event(&mut self, poll_event: PollEvent) -> io::Result<()> {
        match poll_event {
            PollEvent::Readable => self.buffer_rl.read(&mut self.right),
            PollEvent::Writable => self.buffer_lr.write(&mut self.right),
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
    marker: PhantomData<(R, W)>,
}

impl<R: Read, W: Write> Buffer<R, W> {
    /// Create a new, empty buffer
    const fn new() -> Self {
        Self {
            buffer: [0; BUFSIZE],
            start: 0,
            end: 0,
            marker: PhantomData,
        }
    }

    /// Read bytes into the buffer.
    ///
    /// Calling this function will block until `read` is ready to be read.
    fn read(&mut self, read: &mut R) -> io::Result<()> {
        // FIXME: This function will try to read even if the buffer is full. Meaning that in the
        // worst case scenario where `W` is never ready to be written, we will be constantly
        // calling this function. This could be solved by ignoring the event associated with this
        // callback in the dispatcher until `W` is ready.

        // This is the remaining free section that follows the busy section of the buffer.
        let buffer = &mut self.buffer[self.end..];

        // Read `len` bytes from `read` into the buffer.
        let len = read.read(buffer)?;

        // Mark the `len` bytes after the busy section as busy too.
        self.end += len;

        Ok(())
    }

    /// Write bytes from the buffer.
    ///
    /// Calling this function will block until `write` is ready to be written.
    fn write(&mut self, write: &mut W) -> io::Result<()> {
        // FIXME: This function will try to write even if the buffer is empty. Meaning that in the
        // worst case scenario where `R` is never ready to be readn, we will be constantly calling
        // this function. This could be solved by ignoring the event associated with this callback
        // in the dispatcher until `R` is ready.

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

impl<R: Read, W: Write> Default for Buffer<R, W> {
    fn default() -> Self {
        Self::new()
    }
}
