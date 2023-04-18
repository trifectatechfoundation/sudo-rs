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
///   * make it return an Option ("None" if the given fd is not a terminal)
/// - the general idea of a "SafeString" type that clears its memory
///   (although much more robust than in the original code)
///
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, RawFd};
use std::{fs, iter, mem};

use libc::{isatty, tcsetattr, termios, ECHO, ECHONL, TCSANOW};

use sudo_cutils::cerr;

use crate::securemem::PamBuffer;

pub struct HiddenInput {
    fd: RawFd,
    term_orig: termios,
}

impl HiddenInput {
    fn new(tty: &impl AsRawFd) -> io::Result<Option<HiddenInput>> {
        let fd = tty.as_raw_fd();

        // If the file descriptor is not a terminal, there is nothing to hide
        if unsafe { isatty(fd) } == 0 {
            return Ok(None);
        }

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

        Ok(Some(HiddenInput { fd, term_orig }))
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

/// A data structure representing either /dev/tty or /dev/stdin+stderr
pub enum Terminal<'a> {
    Tty(fs::File),
    StdIE(io::StdinLock<'a>, io::StderrLock<'a>),
}

impl Terminal<'_> {
    /// Open the current TTY for user communication
    pub fn open_tty() -> io::Result<Self> {
        Ok(Terminal::Tty(
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/tty")?,
        ))
    }

    /// Open standard input and standard error for user communication
    pub fn open_stdie() -> io::Result<Self> {
        Ok(Terminal::StdIE(io::stdin().lock(), io::stderr().lock()))
    }

    /// Reads input with TTY echo disabled
    pub fn read_password(&mut self) -> io::Result<PamBuffer> {
        let mut input = self.source();
        let _hide_input = HiddenInput::new(&input.as_fd())?;
        read_unbuffered(&mut input)
    }

    /// Reads input with TTY echo enabled
    pub fn read_cleartext(&mut self) -> io::Result<PamBuffer> {
        read_unbuffered(&mut self.source())
    }

    /// Display information
    pub fn prompt(&mut self, text: &str) -> io::Result<()> {
        write_unbuffered(&mut self.sink(), text)
    }

    // boilerplate reduction functions
    fn source(&mut self) -> &mut dyn ReadAsFd {
        match self {
            Terminal::StdIE(x, _) => x,
            Terminal::Tty(x) => x,
        }
    }

    fn sink(&mut self) -> &mut dyn io::Write {
        match self {
            Terminal::StdIE(_, x) => x,
            Terminal::Tty(x) => x,
        }
    }
}

trait ReadAsFd: io::Read + AsFd {}
impl<T: io::Read + AsFd> ReadAsFd for T {}

#[cfg(test)]
mod test {
    use super::{read_unbuffered, write_unbuffered};

    #[test]
    fn miri_test_read() {
        let mut data = "password123\nhello world".as_bytes();
        let buf = read_unbuffered(&mut data).unwrap();
        // check that the \n is not part of input
        assert_eq!(
            buf.iter()
                .map(|&b| b as char)
                .take_while(|&x| x != '\0')
                .collect::<String>(),
            "password123"
        );
        // check that the \n is also consumed but the rest of the input is still there
        assert_eq!(std::str::from_utf8(data).unwrap(), "hello world");
    }

    #[test]
    fn miri_test_write() {
        let mut data = Vec::new();
        write_unbuffered(&mut data, "prompt").unwrap();
        assert_eq!(std::str::from_utf8(&data).unwrap(), "prompt");
    }
}
