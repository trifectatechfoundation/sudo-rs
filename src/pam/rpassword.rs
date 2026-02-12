//! Parts of the code below are Copyright (c) 2023, Conrad Kleinespel et al
//!
//! This module contains code that was originally written by Conrad Kleinespel for the rpassword
//! crate. No copyright notices were found in the original code.
//!
//! See: <https://docs.rs/rpassword/latest/rpassword/>
//!
//! Most code was replaced and so is no longer a derived work; work that we kept:
//!
//! - the "HiddenInput" struct and implementation, with changes:
//!   * replaced occurrences of explicit 'i32' and 'c_int' with RawFd
//!   * open the TTY ourselves to mitigate Linux CVE-2023-2002
//! - the general idea of a "SafeString" type that clears its memory
//!   (although much more robust than in the original code)

use std::ffi::c_void;
use std::io::{self, ErrorKind, Read};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::{fs, mem};

use libc::{ECHO, ECHONL, ICANON, TCSANOW, VEOF, VERASE, VKILL, tcsetattr, termios};

use crate::cutils::{cerr, safe_isatty};
use crate::pam::{PamError, PamResult, askpass};
use crate::system::wait::{Wait, WaitError, WaitOptions};

use super::securemem::PamBuffer;

struct HiddenInput<'a> {
    tty: BorrowedFd<'a>,
    term_orig: termios,
}

impl HiddenInput<'_> {
    fn new(tty: BorrowedFd) -> io::Result<HiddenInput> {
        // Make two copies of the terminal settings. The first one will be modified
        // and the second one will act as a backup for when we want to set the
        // terminal back to its original state.
        let mut term = safe_tcgetattr(tty)?;
        let term_orig = safe_tcgetattr(tty)?;

        // Hide the password. This is what makes this function useful.
        term.c_lflag &= !ECHO;

        // But don't hide the NL character when the user hits ENTER.
        term.c_lflag |= ECHONL;

        // Disable canonical mode to read character by character when pwfeedback is enabled.
        term.c_lflag &= !ICANON;

        // Save the settings for now.
        // SAFETY: we are passing tcsetattr a valid file descriptor and pointer-to-struct
        cerr(unsafe { tcsetattr(tty.as_raw_fd(), TCSANOW, &term) })?;

        Ok(HiddenInput { tty, term_orig })
    }
}

impl Drop for HiddenInput<'_> {
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

fn erase_feedback(sink: &mut dyn io::Write, i: usize) {
    const BACKSPACE: u8 = 0x08;
    for _ in 0..i {
        if sink.write(&[BACKSPACE, b' ', BACKSPACE]).is_err() {
            return;
        }
    }
}

pub(super) enum Hidden<T> {
    No,
    Yes(T),
    WithFeedback(T),
}

/// Heuristically determine the length of the final (potentially incomplete) UTF8 sequence
fn last_char_size(slice: &[u8]) -> usize {
    let start = |byte| byte & 0b1100_0000 == 0b1100_0000;
    let trail = |byte| byte & 0b1100_0000 == 0b1000_0000;

    match slice {
        [] => 0,
        [.., a, b] if start(a) && trail(b) => 2,
        [.., a, b, c] if start(a) && [b, c].into_iter().all(trail) => 3,
        [.., a, b, c, d] if start(a) && [b, c, d].into_iter().all(trail) => 4,
        _ => 1,
    }
}

/// Reads a password from the given file descriptor while optionally showing feedback to the user.
fn read_unbuffered(
    source: &mut dyn io::Read,
    sink: &mut dyn io::Write,
    hide_input: &Hidden<HiddenInput>,
) -> PamResult<PamBuffer> {
    struct Bullets<'a> {
        visible_len: usize,
        enabled: bool,
        sink: &'a mut dyn io::Write,
    }

    const BULLET: &[u8] = b"*";

    impl Bullets<'_> {
        fn push(&mut self) {
            if self.enabled {
                let _ = self.sink.write(BULLET);
                self.visible_len += 1;
            }
        }

        fn pop(&mut self) {
            if self.visible_len > 0 {
                erase_feedback(self.sink, 1);
                self.visible_len -= 1;
            }
        }

        fn clear(&mut self) {
            if self.visible_len > 0 {
                erase_feedback(self.sink, self.visible_len);
                self.visible_len = 0;
            }
        }
    }

    // Ensure we erase the password feedback no matter how we exit read_unbuffered
    impl Drop for Bullets<'_> {
        fn drop(&mut self) {
            self.clear();
            let _ = self.sink.write(b"\n");
        }
    }

    let mut feedback = Bullets {
        visible_len: 0,
        enabled: matches!(hide_input, Hidden::WithFeedback(_)),
        sink,
    };

    let mut password = PamBuffer::default();
    let mut pw_len = 0;

    #[allow(clippy::unbuffered_bytes)]
    for read_byte in source.bytes() {
        let read_byte = read_byte.map_err(|err| match err {
            err if err.kind() == io::ErrorKind::TimedOut => PamError::TimedOut,
            err => PamError::IoError(err),
        })?;

        if read_byte == b'\n' || read_byte == b'\r' {
            return Ok(password);
        }

        if let Hidden::Yes(input) | Hidden::WithFeedback(input) = hide_input {
            if read_byte == input.term_orig.c_cc[VEOF] {
                break;
            }

            if read_byte == input.term_orig.c_cc[VERASE] {
                feedback.pop();
                let chunk = last_char_size(&password[..pw_len]);
                password[pw_len - chunk..pw_len].fill(0);
                pw_len -= chunk;
                continue;
            }

            if read_byte == input.term_orig.c_cc[VKILL] {
                feedback.clear();
                password.fill(0);
                pw_len = 0;
                continue;
            }
        }

        if let Some(dest) = password.get_mut(pw_len) {
            *dest = read_byte;
            pw_len += 1;
            if last_char_size(&password[..pw_len]) == 1 {
                feedback.push();
            }
        } else {
            return Err(PamError::IncorrectPasswordAttempt);
        }
    }

    if pw_len == 0 {
        // In case of EOF or Ctrl-D we don't want to ask for a password a second
        // time, so return an error.
        Err(PamError::NoPasswordProvided)
    } else {
        Ok(password)
    }
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
                    buf.as_mut_ptr() as *mut c_void,
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
    Askpass(PathBuf, io::Sink),
}

impl Terminal<'_> {
    /// Open the current TTY for user communication
    pub fn open_tty() -> PamResult<Self> {
        // control ourselves that we are really talking to a TTY
        // mitigates: https://marc.info/?l=oss-security&m=168164424404224
        Ok(Terminal::Tty(
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/tty")
                .map_err(|_| PamError::TtyRequired)?,
        ))
    }

    /// Open standard input and standard error for user communication
    pub fn open_stdie() -> io::Result<Self> {
        Ok(Terminal::StdIE(io::stdin().lock(), io::stderr().lock()))
    }

    pub fn open_askpass() -> PamResult<Self> {
        let Some(program) = std::env::var_os("SUDO_ASKPASS") else {
            return Err(PamError::NoAskpassProgram);
        };
        let program = PathBuf::from(program);

        if program.is_absolute() {
            Ok(Terminal::Askpass(program, io::sink()))
        } else {
            Err(PamError::InvalidAskpassProgram(program))
        }
    }

    /// Reads input with TTY echo and visual feedback set according to the `hidden` parameter.
    pub(super) fn read_input(
        &mut self,
        prompt: &str,
        timeout: Option<Duration>,
        hidden: Hidden<()>,
    ) -> PamResult<PamBuffer> {
        fn do_hide_input(
            hidden: Hidden<()>,
            input: BorrowedFd,
        ) -> Result<Hidden<HiddenInput>, io::Error> {
            Ok(match hidden {
                // If input is not a tty, we can't hide feedback.
                _ if !safe_isatty(input) => Hidden::No,

                Hidden::No => Hidden::No,
                Hidden::Yes(()) => Hidden::Yes(HiddenInput::new(input)?),
                Hidden::WithFeedback(()) => Hidden::WithFeedback(HiddenInput::new(input)?),
            })
        }

        match self {
            Terminal::StdIE(stdin, stdout) => {
                write_unbuffered(stdout, prompt.as_bytes())?;

                let hide_input = do_hide_input(hidden, stdin.as_fd())?;
                let mut reader = TimeoutRead::new(stdin.as_fd(), timeout);
                read_unbuffered(&mut reader, stdout, &hide_input)
            }
            Terminal::Tty(file) => {
                write_unbuffered(file, prompt.as_bytes())?;

                let hide_input = do_hide_input(hidden, file.as_fd())?;
                let mut reader = TimeoutRead::new(file.as_fd(), timeout);
                read_unbuffered(&mut reader, &mut &*file, &hide_input)
            }
            Terminal::Askpass(program, sink) => {
                let (command_pid, askpass_stdout) = askpass::spawn_askpass(program, prompt)?;

                let mut reader = TimeoutRead::new(askpass_stdout.as_fd(), None);
                let password = read_unbuffered(&mut reader, sink, &Hidden::No)?;

                loop {
                    match command_pid.wait(WaitOptions::new()) {
                        Ok(_) => break,
                        Err(WaitError::Io(err)) if err.kind() == io::ErrorKind::Interrupted => {}
                        Err(WaitError::Io(err)) => return Err(PamError::IoError(err)),
                        Err(WaitError::NotReady) => unreachable!(),
                    }
                }

                Ok(password)
            }
        }
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
    fn sink(&mut self) -> &mut dyn io::Write {
        match self {
            Terminal::StdIE(_, x) => x,
            Terminal::Tty(x) => x,
            Terminal::Askpass(_, x) => x,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn miri_test_read() {
        let mut data = "password123\nhello world".as_bytes();
        let mut stdout = Vec::new();
        let buf = read_unbuffered(&mut data, &mut stdout, &Hidden::No).unwrap();
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
        let mut stdout = Vec::new();
        assert!(read_unbuffered(&mut "a".repeat(511).as_bytes(), &mut stdout, &Hidden::No).is_ok());
        assert!(
            read_unbuffered(&mut "a".repeat(512).as_bytes(), &mut stdout, &Hidden::No).is_err()
        );
    }

    #[test]
    fn miri_test_write() {
        let mut data = Vec::new();
        write_unbuffered(&mut data, b"prompt").unwrap();
        assert_eq!(std::str::from_utf8(&data).unwrap(), "prompt");
    }
}
