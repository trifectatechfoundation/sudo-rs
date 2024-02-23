//! This module is a port of ogsudo's `lib/util/term.c` with some minor changes to make it
//! rust-like.

use std::{
    ffi::c_int,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    mem::MaybeUninit,
    os::fd::{AsRawFd, RawFd},
    sync::atomic::{AtomicBool, Ordering},
};

use libc::{
    c_void, cfgetispeed, cfgetospeed, cfmakeraw, cfsetispeed, cfsetospeed, ioctl, sigaction,
    sigemptyset, sighandler_t, siginfo_t, sigset_t, tcflag_t, tcgetattr, tcsetattr, termios,
    winsize, CS7, CS8, ECHO, ECHOCTL, ECHOE, ECHOK, ECHOKE, ECHONL, ICANON, ICRNL, IEXTEN, IGNCR,
    IGNPAR, IMAXBEL, INLCR, INPCK, ISIG, ISTRIP, IUTF8, IXANY, IXOFF, IXON, NOFLSH, OCRNL, OLCUC,
    ONLCR, ONLRET, ONOCR, OPOST, PARENB, PARMRK, PARODD, PENDIN, SIGTTOU, TCSADRAIN, TCSAFLUSH,
    TIOCGWINSZ, TIOCSWINSZ, TOSTOP,
};

use super::{TermSize, Terminal};
use crate::{
    cutils::cerr,
    system::{interface::ProcessId, make_zeroed_sigaction},
};

const INPUT_FLAGS: tcflag_t = IGNPAR
    | PARMRK
    | INPCK
    | ISTRIP
    | INLCR
    | IGNCR
    | ICRNL
    // | IUCLC /* FIXME: not in libc */
    | IXON
    | IXANY
    | IXOFF
    | IMAXBEL
    | IUTF8;
const OUTPUT_FLAGS: tcflag_t = OPOST | OLCUC | ONLCR | OCRNL | ONOCR | ONLRET;
const CONTROL_FLAGS: tcflag_t = CS7 | CS8 | PARENB | PARODD;
const LOCAL_FLAGS: tcflag_t = ISIG
    | ICANON
    // | XCASE /* FIXME: not in libc */
    | ECHO
    | ECHOE
    | ECHOK
    | ECHONL
    | NOFLSH
    | TOSTOP
    | IEXTEN
    | ECHOCTL
    | ECHOKE
    | PENDIN;

static GOT_SIGTTOU: AtomicBool = AtomicBool::new(false);

extern "C" fn on_sigttou(_signal: c_int, _info: *mut siginfo_t, _: *mut c_void) {
    GOT_SIGTTOU.store(true, Ordering::SeqCst);
}

/// This is like `tcsetattr` but it only suceeds if we are in the foreground process group.
fn tcsetattr_nobg(fd: c_int, flags: c_int, tp: *const termios) -> io::Result<()> {
    // This function is based around the fact that we receive `SIGTTOU` if we call `tcsetattr` and
    // we are not in the foreground process group.

    let mut original_action = MaybeUninit::<sigaction>::uninit();

    let action = {
        let mut raw: libc::sigaction = make_zeroed_sigaction();
        // Call `on_sigttou` if `SIGTTOU` arrives.
        raw.sa_sigaction = on_sigttou as sighandler_t;
        // Exclude any other signals from the set
        raw.sa_mask = {
            let mut sa_mask = MaybeUninit::<sigset_t>::uninit();
            unsafe { sigemptyset(sa_mask.as_mut_ptr()) };
            unsafe { sa_mask.assume_init() }
        };
        raw.sa_flags = 0;
        raw.sa_restorer = None;
        raw
    };
    // Reset `GOT_SIGTTOU`.
    GOT_SIGTTOU.store(false, Ordering::SeqCst);
    // Set `action` as the action for `SIGTTOU` and store the original action in `original_action`
    // to restore it later.
    unsafe { sigaction(SIGTTOU, &action, original_action.as_mut_ptr()) };
    // Call `tcsetattr` until it suceeds and ignore interruptions if we did not receive `SIGTTOU`.
    let result = loop {
        match cerr(unsafe { tcsetattr(fd, flags, tp) }) {
            Ok(_) => break Ok(()),
            Err(err) => {
                let got_sigttou = GOT_SIGTTOU.load(Ordering::SeqCst);
                if got_sigttou || err.kind() != io::ErrorKind::Interrupted {
                    break Err(err);
                }
            }
        }
    };
    // Restore the original action.
    unsafe { sigaction(SIGTTOU, original_action.as_ptr(), std::ptr::null_mut()) };

    result
}

/// Type to manipulate the settings of the user's terminal.
pub struct UserTerm {
    tty: File,
    original_termios: MaybeUninit<termios>,
    changed: bool,
}

impl UserTerm {
    /// Open the user's terminal.
    pub fn open() -> io::Result<Self> {
        Ok(Self {
            tty: OpenOptions::new().read(true).write(true).open("/dev/tty")?,
            original_termios: MaybeUninit::uninit(),
            changed: false,
        })
    }

    pub(crate) fn get_size(&self) -> io::Result<TermSize> {
        let mut term_size = MaybeUninit::<TermSize>::uninit();

        cerr(unsafe {
            ioctl(
                self.tty.as_raw_fd(),
                TIOCGWINSZ,
                term_size.as_mut_ptr().cast::<winsize>(),
            )
        })?;

        Ok(unsafe { term_size.assume_init() })
    }

    /// Copy the settings of the user's terminal to the `dst` terminal.
    pub fn copy_to<D: AsRawFd>(&self, dst: &D) -> io::Result<()> {
        let src = self.tty.as_raw_fd();
        let dst = dst.as_raw_fd();

        let mut tt_src = MaybeUninit::<termios>::uninit();
        let mut tt_dst = MaybeUninit::<termios>::uninit();
        let mut wsize = MaybeUninit::<winsize>::uninit();

        cerr(unsafe { tcgetattr(src, tt_src.as_mut_ptr()) })?;
        cerr(unsafe { tcgetattr(dst, tt_dst.as_mut_ptr()) })?;

        let tt_src = unsafe { tt_src.assume_init() };
        let mut tt_dst = unsafe { tt_dst.assume_init() };

        // Clear select input, output, control and local flags.
        tt_dst.c_iflag &= !INPUT_FLAGS;
        tt_dst.c_oflag &= !OUTPUT_FLAGS;
        tt_dst.c_cflag &= !CONTROL_FLAGS;
        tt_dst.c_lflag &= !LOCAL_FLAGS;

        // Copy select input, output, control and local flags.
        tt_dst.c_iflag |= tt_src.c_iflag & INPUT_FLAGS;
        tt_dst.c_oflag |= tt_src.c_oflag & OUTPUT_FLAGS;
        tt_dst.c_cflag |= tt_src.c_cflag & CONTROL_FLAGS;
        tt_dst.c_lflag |= tt_src.c_lflag & LOCAL_FLAGS;

        // Copy special chars from src verbatim.
        tt_dst.c_cc.copy_from_slice(&tt_src.c_cc);

        // Copy speed from `src`.
        {
            let mut speed = unsafe { cfgetospeed(&tt_src) };
            // Zero output speed closes the connection.
            if speed == libc::B0 {
                speed = libc::B38400;
            }
            unsafe { cfsetospeed(&mut tt_dst, speed) };
            speed = unsafe { cfgetispeed(&tt_src) };
            unsafe { cfsetispeed(&mut tt_dst, speed) };
        }

        tcsetattr_nobg(dst, TCSAFLUSH, &tt_dst)?;

        cerr(unsafe { ioctl(src, TIOCGWINSZ, &mut wsize) })?;
        cerr(unsafe { ioctl(dst, TIOCSWINSZ, &wsize) })?;

        Ok(())
    }

    /// Set the user's terminal to raw mode. Enable terminal signals if `with_signals` is set to
    /// `true`.
    pub fn set_raw_mode(&mut self, with_signals: bool) -> io::Result<()> {
        let fd = self.tty.as_raw_fd();

        if !self.changed {
            cerr(unsafe { tcgetattr(fd, self.original_termios.as_mut_ptr()) })?;
        }
        // Retrieve the original terminal.
        let mut term = unsafe { self.original_termios.assume_init() };
        // Set terminal to raw mode.
        unsafe { cfmakeraw(&mut term) };
        // Enable terminal signals.
        if with_signals {
            term.c_cflag |= ISIG;
        }

        tcsetattr_nobg(fd, TCSADRAIN, &term)?;
        self.changed = true;

        Ok(())
    }

    /// Restore the saved terminal settings if we are in the foreground process group.
    ///
    /// This change is done after waiting for all the queued output to be written. To discard the
    /// queued input `flush` must be set to `true`.
    pub fn restore(&mut self, flush: bool) -> io::Result<()> {
        if self.changed {
            let fd = self.tty.as_raw_fd();
            let flags = if flush { TCSAFLUSH } else { TCSADRAIN };
            tcsetattr_nobg(fd, flags, self.original_termios.as_ptr())?;
            self.changed = false;
        }

        Ok(())
    }

    /// This is like `tcsetpgrp` but it only suceeds if we are in the foreground process group.
    pub fn tcsetpgrp_nobg(&self, pgrp: ProcessId) -> io::Result<()> {
        // This function is based around the fact that we receive `SIGTTOU` if we call `tcsetpgrp` and
        // we are not in the foreground process group.

        let mut original_action = MaybeUninit::<sigaction>::uninit();

        let action = {
            let mut raw: libc::sigaction = make_zeroed_sigaction();
            // Call `on_sigttou` if `SIGTTOU` arrives.
            raw.sa_sigaction = on_sigttou as sighandler_t;
            // Exclude any other signals from the set
            raw.sa_mask = {
                let mut sa_mask = MaybeUninit::<sigset_t>::uninit();
                unsafe { sigemptyset(sa_mask.as_mut_ptr()) };
                unsafe { sa_mask.assume_init() }
            };
            raw.sa_flags = 0;
            raw.sa_restorer = None;
            raw
        };
        // Reset `GOT_SIGTTOU`.
        GOT_SIGTTOU.store(false, Ordering::SeqCst);
        // Set `action` as the action for `SIGTTOU` and store the original action in `original_action`
        // to restore it later.
        unsafe { sigaction(SIGTTOU, &action, original_action.as_mut_ptr()) };
        // Call `tcsetattr` until it suceeds and ignore interruptions if we did not receive `SIGTTOU`.
        let result = loop {
            match self.tty.tcsetpgrp(pgrp) {
                Ok(()) => break Ok(()),
                Err(err) => {
                    let got_sigttou = GOT_SIGTTOU.load(Ordering::SeqCst);
                    if got_sigttou || err.kind() != io::ErrorKind::Interrupted {
                        break Err(err);
                    }
                }
            }
        };
        // Restore the original action.
        unsafe { sigaction(SIGTTOU, original_action.as_ptr(), std::ptr::null_mut()) };

        result
    }
}

impl AsRawFd for UserTerm {
    fn as_raw_fd(&self) -> RawFd {
        self.tty.as_raw_fd()
    }
}

impl Read for UserTerm {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.tty.read(buf)
    }
}

impl Write for UserTerm {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.tty.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.tty.flush()
    }
}
