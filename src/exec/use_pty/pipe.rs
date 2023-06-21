use std::{
    io::{Read, Write},
    marker::PhantomData,
};

/// The size of the internal buffer of the pipe.
const BUFSIZE: usize = 6 * 1024;

/// A pipe to connect a [`Read`] to a [`Write`].
///
/// This type uses an internal buffer so it can store the bytes read from `R` before they are
/// written to `W`.
pub(super) struct Pipe<R: Read, W: Write> {
    buffer: [u8; BUFSIZE],
    // The start of the busy section of the buffer.
    start: usize,
    // The end of the busy section of the buffer.
    end: usize,
    marker: PhantomData<(R, W)>,
}

impl<R: Read, W: Write> Pipe<R, W> {
    pub(super) const fn new() -> Self {
        Self {
            buffer: [0; BUFSIZE],
            start: 0,
            end: 0,
            marker: PhantomData,
        }
    }
    /// Read bytes into the internal buffer of the pipe.
    ///
    /// Calling this function will block until `read` is ready to be read.
    pub(super) fn on_read(&mut self, read: &mut R) {
        // FIXME: This function will try to read even if the internal buffer is full. Meaning that
        // in the worst case scenario where `W` is never ready to be written, we will be constantly
        // calling this function. This could be solved by ignoring the event associated with this
        // callback in the dispatcher until `W` is ready.

        // This is the remaining free section that follows the busy section of the buffer.
        let buffer = &mut self.buffer[self.end..];

        // Read `len` bytes from `read` into the buffer.
        let Ok(len) = read.read(buffer) else {
            return;
        };

        // Mark the `len` bytes after the busy section as busy too.
        self.end += len;
    }

    /// Write bytes from the internal buffer of the pipe.
    ///
    /// Calling this function will block until `write` is ready to be written.
    pub(super) fn on_write(&mut self, write: &mut W) {
        // FIXME: This function will try to write even if the internal buffer is empty. Meaning that
        // in the worst case scenario where `R` is never ready to be readn, we will be constantly
        // calling this function. This could be solved by ignoring the event associated with this
        // callback in the dispatcher until `R` is ready.

        // This is the busy section of the buffer.
        let buffer = &self.buffer[self.start..self.end];

        // Write the first `len` bytes of the busy section to `write`.
        let Ok(len) = write.write(buffer) else {
            return;
        };

        if len == buffer.len() {
            // If we were able to write all the busy section, we can mark the whole buffer as free.
            self.start = 0;
            self.end = 0;
        } else {
            // Otherwise we just free the first `len` bytes of the busy section.
            self.start += len;
        }
    }
}

impl<R: Read, W: Write> Default for Pipe<R, W> {
    fn default() -> Self {
        Self::new()
    }
}
