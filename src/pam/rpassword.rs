/// Parts of the code below are Copyright (c) 2023, Conrad Kleinespel et al
///
/// This module contains code that was originally written by Conrad Kleinespel for the rpassword
/// crate. No copyright notices were found in the original code.
///
/// See: https://docs.rs/rpassword/latest/rpassword/
///
/// Most code was replaced and so is no longer a derived work; work that we kept:
///
/// - the "HiddenInput" struct and implementation, with changes:
///   * replaced occurrences of explicit 'i32' and 'c_int' with RawFd
///   * open the TTY ourselves to mitigate Linux CVE-2023-2002
/// - the general idea of a "SafeString" type that clears its memory
///   (although much more robust than in the original code)
///
use std::io::{self, Error, ErrorKind, Read};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::time::{Duration, Instant};
use std::{fs, mem};

use libc::{tcsetattr, termios, ECHO, ECHONL, ICANON, TCSANOW, VEOF, VERASE, VKILL};

use crate::cutils::cerr;

use super::securemem::PamBuffer;

struct HiddenInput {
    tty: fs::File,
    term_orig: termios,
}

impl HiddenInput {
    fn new(feedback: bool) -> io::Result<Option<HiddenInput>> {
        // control ourselves that we are really talking to a TTY
        // mitigates: https://marc.info/?l=oss-security&m=168164424404224
        let Ok(tty) = fs::File::open("/dev/tty") else {
            // if we have nothing to show, we have nothing to hide
            return Ok(None);
        };

        // Make two copies of the terminal settings. The first one will be modified
        // and the second one will act as a backup for when we want to set the
        // terminal back to its original state.
        let mut term = safe_tcgetattr(&tty)?;
        let term_orig = safe_tcgetattr(&tty)?;

        // Hide the password. This is what makes this function useful.
        term.c_lflag &= !ECHO;

        // But don't hide the NL character when the user hits ENTER.
        term.c_lflag |= ECHONL;

        if feedback {
            // Disable canonical mode to read character by character when pwfeedback is enabled.
            term.c_lflag &= !ICANON;
        }

        // Save the settings for now.
        // SAFETY: we are passing tcsetattr a valid file descriptor and pointer-to-struct
        cerr(unsafe { tcsetattr(tty.as_raw_fd(), TCSANOW, &term) })?;

        Ok(Some(HiddenInput { tty, term_orig }))
    }
}

impl Drop for HiddenInput {
    fn drop(&mut self) {
        // Set the the mode back to normal
        // SAFETY: we are passing tcsetattr a valid file descriptor and pointer-to-struct
        unsafe {
            tcsetattr(self.tty.as_raw_fd(), TCSANOW, &self.term_orig);
        }
    }
}

fn safe_tcgetattr(tty: impl AsFd) -> io::Result<termios> {
    let mut term = mem::MaybeUninit::<termios>::uninit();
    // SAFETY: we are passing tcgetattr a pointer to valid memory
    cerr(unsafe { ::libc::tcgetattr(tty.as_fd().as_raw_fd(), term.as_mut_ptr()) })?;
    // SAFETY: if the previous call was a success, `tcgetattr` has initialized `term`
    Ok(unsafe { term.assume_init() })
}

/// Reads a password from the given file descriptor
fn read_unbuffered(source: &mut dyn io::Read) -> io::Result<PamBuffer> {
    let mut password = PamBuffer::default();
    let mut pwd_iter = password.iter_mut();

    const EOL: u8 = 0x0A;
    //TODO: we actually only want to allow clippy::unbuffered_bytes
    #[allow(clippy::perf)]
    let input = source.bytes().take_while(|x| x.as_ref().ok() != Some(&EOL));

    for read_byte in input {
        if let Some(dest) = pwd_iter.next() {
            *dest = read_byte?
        } else {
            return Err(Error::new(
                ErrorKind::OutOfMemory,
                "incorrect password attempt",
            ));
        }
    }

    Ok(password)
}

fn erase_feedback(sink: &mut dyn io::Write, i: usize) {
    const BACKSPACE: u8 = 0x08;
    for _ in 0..i {
        if sink.write(&[BACKSPACE, b' ', BACKSPACE]).is_err() {
            return;
        }
    }
}

/// Reads a password from the given file descriptor while showing feedback to the user.
fn read_unbuffered_with_feedback(
    source: &mut dyn io::Read,
    sink: &mut dyn io::Write,
    hide_input: &HiddenInput,
) -> io::Result<PamBuffer> {
    let mut password = PamBuffer::default();
    let mut pw_len = 0;

    // invariant: the amount of nonzero-bytes in the buffer correspond
    // with the amount of asterisks on the terminal (both tracked in `pw_len`)
    //TODO: we actually only want to allow clippy::unbuffered_bytes
    #[allow(clippy::perf)]
    for read_byte in source.bytes() {
        let read_byte = read_byte?;

        if read_byte == b'\n' || read_byte == b'\r' {
            erase_feedback(sink, pw_len);
            let _ = sink.write(b"\n");
            break;
        }

        if read_byte == hide_input.term_orig.c_cc[VEOF] {
            erase_feedback(sink, pw_len);
            password.fill(0);
            break;
        }

        if read_byte == hide_input.term_orig.c_cc[VERASE] {
            if pw_len > 0 {
                erase_feedback(sink, 1);
                password[pw_len - 1] = 0;
                pw_len -= 1;
            }
        } else if read_byte == hide_input.term_orig.c_cc[VKILL] {
            erase_feedback(sink, pw_len);
            password.fill(0);
            pw_len = 0;
        } else {
            #[allow(clippy::collapsible_else_if)]
            if let Some(dest) = password.get_mut(pw_len) {
                *dest = read_byte;
                pw_len += 1;
                let _ = sink.write(b"*");
            } else {
                erase_feedback(sink, pw_len);

                return Err(Error::new(
                    ErrorKind::OutOfMemory,
                    "incorrect password attempt",
                ));
            }
        }
    }

    Ok(password)
}

/// Write something and immediately flush
fn write_unbuffered(sink: &mut dyn io::Write, text: &[u8]) -> io::Result<()> {
    sink.write_all(text)?;
    sink.flush()
}

struct TimeoutRead<'a> {
    timeout_at: Option<Instant>,
    fd: BorrowedFd<'a>,
}

impl<'a> TimeoutRead<'a> {
    fn new(fd: BorrowedFd<'a>, timeout: Option<Duration>) -> TimeoutRead<'a> {
        TimeoutRead {
            timeout_at: timeout.map(|timeout| Instant::now() + timeout),
            fd,
        }
    }
}

impl io::Read for TimeoutRead<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let pollmask = libc::POLLIN | libc::POLLRDHUP;

        let mut pollfd = [libc::pollfd {
            fd: self.fd.as_raw_fd(),
            events: pollmask,
            revents: 0,
        }; 1];

        let timeout = match self.timeout_at {
            Some(timeout_at) => {
                let now = Instant::now();
                if now > timeout_at {
                    return Err(io::Error::from(ErrorKind::TimedOut));
                }

                (timeout_at - now)
                    .as_millis()
                    .try_into()
                    .unwrap_or(i32::MAX)
            }
            None => -1,
        };

        // SAFETY: pollfd is initialized and its length matches
        cerr(unsafe {
            libc::poll(
                pollfd.as_mut_ptr(),
                pollfd.len().try_into().unwrap(),
                timeout,
            )
        })?;

        // There may yet be data waiting to be read even if POLLHUP is set.
        if pollfd[0].revents & (pollmask | libc::POLLHUP) > 0 {
            // SAFETY: buf is initialized and its length matches
            let ret = cerr(unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            })?;

            Ok(ret as usize)
        } else {
            Err(io::Error::from(io::ErrorKind::TimedOut))
        }
    }
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
    pub fn read_password(&mut self, timeout: Option<Duration>) -> io::Result<PamBuffer> {
        let mut input = self.source_timeout(timeout);
        let _hide_input = HiddenInput::new(false)?;
        read_unbuffered(&mut input)
    }

    /// Reads input with TTY echo disabled, but do provide visual feedback while typing.
    pub fn read_password_with_feedback(
        &mut self,
        timeout: Option<Duration>,
    ) -> io::Result<PamBuffer> {
        match (HiddenInput::new(true)?, self) {
            (Some(hide_input), Terminal::StdIE(stdin, stdout)) => {
                let mut reader = TimeoutRead::new(stdin.as_fd(), timeout);
                read_unbuffered_with_feedback(&mut reader, stdout, &hide_input)
            }
            (Some(hide_input), Terminal::Tty(file)) => {
                let mut reader = TimeoutRead::new(file.as_fd(), timeout);
                read_unbuffered_with_feedback(&mut reader, &mut &*file, &hide_input)
            }
            (None, term) => read_unbuffered(&mut term.source_timeout(timeout)),
        }
    }

    /// Reads input with TTY echo enabled
    pub fn read_cleartext(&mut self) -> io::Result<PamBuffer> {
        read_unbuffered(self.source())
    }

    /// Display information
    pub fn prompt(&mut self, text: &str) -> io::Result<()> {
        write_unbuffered(self.sink(), text.as_bytes())
    }

    /// Ring the bell
    pub fn bell(&mut self) -> io::Result<()> {
        const BELL: &[u8; 1] = b"\x07";
        write_unbuffered(self.sink(), BELL)
    }

    // boilerplate reduction functions
    fn source(&mut self) -> &mut dyn io::Read {
        match self {
            Terminal::StdIE(x, _) => x,
            Terminal::Tty(x) => x,
        }
    }

    fn source_timeout(&self, timeout: Option<Duration>) -> TimeoutRead<'_> {
        match self {
            Terminal::StdIE(stdin, _) => TimeoutRead::new(stdin.as_fd(), timeout),
            Terminal::Tty(file) => TimeoutRead::new(file.as_fd(), timeout),
        }
    }

    fn sink(&mut self) -> &mut dyn io::Write {
        match self {
            Terminal::StdIE(_, x) => x,
            Terminal::Tty(x) => x,
        }
    }
}

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
    fn miri_test_longpwd() {
        assert!(read_unbuffered(&mut "a".repeat(511).as_bytes()).is_ok());
        assert!(read_unbuffered(&mut "a".repeat(512).as_bytes()).is_err());
    }

    #[test]
    fn miri_test_write() {
        let mut data = Vec::new();
        write_unbuffered(&mut data, b"prompt").unwrap();
        assert_eq!(std::str::from_utf8(&data).unwrap(), "prompt");
    }
}
