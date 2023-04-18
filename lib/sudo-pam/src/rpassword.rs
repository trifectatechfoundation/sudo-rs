/// Derived work from rpassword and rtoolbox, Copyright (c) 2023, Conrad Kleinespel et al
///
/// This module contains code that was originally written by Conrad Kleinespel for the rpassword
/// crate. No copyright notices were found in the original code.

/// See: https://docs.rs/rpassword/latest/rpassword/
///
/// CHANGES TO THE ORIGINAL CODE:
/// - {prompt,read}_password_from_bufread deleted (we don't need them)
/// - rtool_box::print_tty was inlined
/// - SafeString was removed and replaced with more general PamBuffer type (and moved to cutils);
///   also, the original code actually allowed the string to quickly escape the security net
/// - instead of String, password are read as [u8]
/// - this also removes the need for 'fix_line_issues', since we know we read a \n
/// - replaced 'io_result' with our own 'cerr' function.
/// - replaced occurences of explicit 'i32' and 'c_int' with RawFd
/// - unified 'read_password' and 'read_password_from_fd_with_hidden_input' functions
/// - only open /dev/tty once
use std::io::{self, Read, Write};
use std::mem;
use std::os::fd::{AsRawFd, RawFd};

use libc::{tcsetattr, termios, ECHO, ECHONL, TCSANOW};

use sudo_cutils::cerr;

use crate::securemem::PamBuffer;

struct HiddenInput {
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
fn read_password(source: &mut (impl io::Read + AsRawFd)) -> io::Result<PamBuffer> {
    let _hide_input = HiddenInput::new(source)?;

    let mut password = PamBuffer::default();

    const EOL: u8 = 0x0A;

    for (read_byte, dest) in std::iter::zip(source.bytes(), password.iter_mut()) {
        match read_byte? {
            EOL => break,
            ch => *dest = ch,
        }
    }

    Ok(password)
}

/// Prompts on the TTY and then reads a password from TTY
pub fn prompt_password(prompt: impl ToString) -> io::Result<PamBuffer> {
    let mut stream = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")?;
    stream
        .write_all(prompt.to_string().as_str().as_bytes())
        .and_then(|_| stream.flush())
        .and_then(|_| read_password(&mut stream))
}
