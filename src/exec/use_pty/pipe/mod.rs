mod ring_buffer;

use std::{
    io::{self, Read, Write},
    marker::PhantomData,
    os::fd::AsFd,
};

use crate::exec::event::{EventHandle, EventRegistry, PollEvent, Process};

use self::ring_buffer::RingBuffer;

// A pipe able to stream data bidirectionally between two read-write types.
pub(super) struct Pipe<L, R> {
    left: L,
    right: R,
    buffer_lr: Buffer<L, R>,
    buffer_rl: Buffer<R, L>,
    background: bool,
}

impl<L: Read + Write + AsFd, R: Read + Write + AsFd> Pipe<L, R> {
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
            background: false,
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

    /// Stop the poll events of the left end of this pipe.
    pub(super) fn disable_input<T: Process>(&mut self, registry: &mut EventRegistry<T>) {
        self.buffer_lr.read_handle.ignore(registry);
        self.background = true;
    }

    /// Resume the poll events of this pipe
    pub(super) fn resume_events<T: Process>(&mut self, registry: &mut EventRegistry<T>) {
        if !self.background {
            self.buffer_lr.read_handle.resume(registry);
        }
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
            PollEvent::Writable => {
                if self.buffer_rl.write(&mut self.left, registry)? {
                    self.buffer_rl.read_handle.resume(registry);
                }
                Ok(())
            }
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
            PollEvent::Writable => {
                if self.buffer_lr.write(&mut self.right, registry)? && !self.background {
                    self.buffer_lr.read_handle.resume(registry);
                }

                Ok(())
            }
        }
    }

    /// Ensure that all the contents of the pipe's internal buffer are written to the left side.
    pub(super) fn flush_left(&mut self) -> io::Result<()> {
        self.buffer_rl.flush(&mut self.left)
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
    ) -> io::Result<bool> {
        // If the buffer is empty, there is nothing to be written.
        if self.internal.is_empty() {
            self.write_handle.ignore(registry);
            return Ok(false);
        }

        // Remove bytes from the buffer and write them.
        let removed_len = self.internal.remove(write)?;

        // Return whether we actually freed up some buffer space
        Ok(removed_len > 0)
    }

    /// Flush this buffer, ensuring that all the contents of its internal buffer are written.
    fn flush(&mut self, write: &mut W) -> io::Result<()> {
        // Remove bytes from the buffer and write them.
        self.internal.remove(write)?;

        write.flush()
    }
}
