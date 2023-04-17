/// Derived work from rpassword and rtoolbox, Copyright (c) 2023, Conrad Kleinespel et al
///
/// This module contains code that was originally written by Conrad Kleinespel for the rpassword
/// crate. No copyright notices were found in the original code.
///
/// See: https://docs.rs/rpassword/latest/rpassword/
///
/// Most code was replaced and so is no longer a derived work; work that we kept:
///
/// - the "HiddenInput" struct and implementation
///   * replaced occurences of explicit 'i32' and 'c_int' with RawFd
/// - the general idea of a "SafeString" type that clears its memory
///   (although much more robust than in the original code)
///
use std::io::{self, Read};
use std::os::fd::{AsRawFd, RawFd};
use std::{fs, iter, mem};

use libc::{tcsetattr, termios, ECHO, ECHONL, TCSANOW};

use sudo_cutils::cerr;

use crate::securemem::PamBuffer;

pub struct HiddenInput {
    fd: RawFd,
    term_orig: termios,
}

impl HiddenInput {
    fn new(tty: &impl AsRawFd) -> io::Result<HiddenInput> {
        let fd = tty.as_raw_fd();

        // Make two copies of the terminal settings. The first one will be modified
        // and the second one will act as a backup for when we want to set the
        // terminal back to its original state.
        let mut term = safe_tcgetattr(fd)?;
        let term_orig = safe_tcgetattr(fd)?;

        // Hide the password. This is what makes this function useful.
        term.c_lflag &= !ECHO;

        // But don't hide the NL character when the user hits ENTER.
        term.c_lflag |= ECHONL;

        // Save the settings for now.
        cerr(unsafe { tcsetattr(fd, TCSANOW, &term) })?;

        Ok(HiddenInput { fd, term_orig })
    }
}

impl Drop for HiddenInput {
    fn drop(&mut self) {
        // Set the the mode back to normal
        unsafe {
            tcsetattr(self.fd, TCSANOW, &self.term_orig);
        }
    }
}

fn safe_tcgetattr(fd: RawFd) -> io::Result<termios> {
    let mut term = mem::MaybeUninit::<termios>::uninit();
    cerr(unsafe { ::libc::tcgetattr(fd, term.as_mut_ptr()) })?;
    Ok(unsafe { term.assume_init() })
}

/// Reads a password from the given file descriptor
fn read_unbuffered(source: &mut impl io::Read) -> io::Result<PamBuffer> {
    let mut password = PamBuffer::default();

    const EOL: u8 = 0x0A;

    for (read_byte, dest) in iter::zip(source.bytes(), password.iter_mut()) {
        match read_byte? {
            EOL => break,
            ch => *dest = ch,
        }
    }

    Ok(password)
}

/// Write something and immediately flush
fn write_unbuffered(sink: &mut impl io::Write, text: &str) -> io::Result<()> {
    sink.write_all(text.as_bytes())?;
    sink.flush()
}

/// Open the TTY
pub fn open_tty() -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
}

pub trait Terminal {
    fn read_password(&mut self) -> io::Result<PamBuffer>;
    fn read_cleartext(&mut self) -> io::Result<PamBuffer>;
    fn prompt(&mut self, prompt: &str) -> io::Result<()>;
}

impl<TTY: io::Write + io::Read + AsRawFd> Terminal for TTY {
    /// Prompts on the given device and then reads input with TTY echo disabled
    fn read_password(&mut self) -> io::Result<PamBuffer> {
        let _hide_input = HiddenInput::new(self)?;
        read_unbuffered(self)
    }

    /// Prompts and reads from the given device
    fn read_cleartext(&mut self) -> io::Result<PamBuffer> {
        read_unbuffered(self)
    }

    /// Only display information
    fn prompt(&mut self, prompt: &str) -> io::Result<()> {
        write_unbuffered(self, &prompt)
    }
}

pub struct StdIO;

// For the case where "sudo -S" is used, use "stdin" and "stderr" instead.
impl Terminal for StdIO {
    fn read_password(&mut self) -> io::Result<PamBuffer> {
        let mut source = io::stdin().lock();
        let _hide_input = HiddenInput::new(&source)?;
        read_unbuffered(&mut source)
    }

    fn read_cleartext(&mut self) -> io::Result<PamBuffer> {
        read_unbuffered(&mut io::stdin().lock())
    }

    fn prompt(&mut self, prompt: &str) -> io::Result<()> {
        write_unbuffered(&mut io::stderr().lock(), &prompt)
    }
}
