//! This module is a port of ogsudo's `lib/util/term.c` with some minor changes to make it
//! rust-like.

use std::{
    ffi::{c_int, c_void},
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    mem::MaybeUninit,
    os::fd::{AsFd, AsRawFd, BorrowedFd},
    sync::atomic::{AtomicBool, Ordering},
};

use libc::{
    CS7, CS8, ECHO, ECHOCTL, ECHOE, ECHOK, ECHOKE, ECHONL, ICANON, ICRNL, IEXTEN, IGNCR, IGNPAR,
    IMAXBEL, INLCR, INPCK, ISIG, ISTRIP, IXANY, IXOFF, IXON, NOFLSH, OCRNL, ONLCR, ONLRET, ONOCR,
    OPOST, PARENB, PARMRK, PARODD, PENDIN, SIGTTOU, TCSADRAIN, TCSAFLUSH, TIOCGWINSZ, TIOCSWINSZ,
    TOSTOP, cfgetispeed, cfgetospeed, cfmakeraw, cfsetispeed, cfsetospeed, ioctl, sigaction,
    sigemptyset, sighandler_t, siginfo_t, sigset_t, tcflag_t, tcgetattr, tcsetattr, termios,
    winsize,
};
#[cfg(target_os = "linux")]
use libc::{IUTF8, OLCUC};

#[cfg(not(target_os = "linux"))]
const IUTF8: libc::tcflag_t = 0;
#[cfg(not(target_os = "linux"))]
const OLCUC: libc::tcflag_t = 0;

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

/// This is like `tcsetattr` but it only succeeds if we are in the foreground process group.
/// # Safety
///
/// The arguments to this function have to be valid arguments to `tcsetattr`.
unsafe fn tcsetattr_nobg(fd: c_int, flags: c_int, tp: *const termios) -> io::Result<()> {
    // This function is based around the fact that we receive `SIGTTOU` if we call `tcsetattr` and
    // we are not in the foreground process group.

    // SAFETY: is the responsibility of the caller of `tcsetattr_nobg`
    let setattr = || cerr(unsafe { tcsetattr(fd, flags, tp) }).map(|_| ());

    catching_sigttou(setattr)
}

fn catching_sigttou(mut function: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    extern "C" fn on_sigttou(_signal: c_int, _info: *mut siginfo_t, _: *mut c_void) {
        GOT_SIGTTOU.store(true, Ordering::SeqCst);
    }

    let action = {
        let mut raw: libc::sigaction = make_zeroed_sigaction();
        // Call `on_sigttou` if `SIGTTOU` arrives.
        raw.sa_sigaction = on_sigttou as *const () as sighandler_t;
        // Exclude any other signals from the set
        raw.sa_mask = {
            let mut sa_mask = MaybeUninit::<sigset_t>::uninit();
            // SAFETY: sa_mask is a valid and dereferenceble pointer; it will
            // become initialized by `sigemptyset`
            unsafe {
                sigemptyset(sa_mask.as_mut_ptr());
                sa_mask.assume_init()
            }
        };
        raw.sa_flags = 0;
        raw
    };
    // Reset `GOT_SIGTTOU`.
    GOT_SIGTTOU.store(false, Ordering::SeqCst);

    // Set `action` as the action for `SIGTTOU` and store the original action in `original_action`
    // to restore it later.
    //
    // SAFETY: `original_action` is a valid pointer; second, the `action` installed (on_sigttou):
    // - is itself a safe function
    // - only updates an atomic variable, so cannot violate memory unsafety that way
    // - doesn't call any async-unsafe functions (refer to signal-safety(7))
    // Therefore it can safely be installed as a signal handler.
    // Furthermore, `sigaction` will initialize `original_action`.
    let original_action = unsafe {
        let mut original_action = MaybeUninit::<sigaction>::uninit();
        sigaction(SIGTTOU, &action, original_action.as_mut_ptr());
        original_action.assume_init()
    };

    // Call `tcsetattr` until it suceeds and ignore interruptions if we did not receive `SIGTTOU`.
    let result = loop {
        match function() {
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
    //
    // SAFETY: `original_action` is a valid pointer, and was initialized by the preceding
    // call to `sigaction` (and not subsequently altered, since it is not mut). The third parameter
    // is allowed to be NULL (this means we ignore the previously-installed handler)
    unsafe { sigaction(SIGTTOU, &original_action, std::ptr::null_mut()) };

    result
}

/// Type to manipulate the settings of the user's terminal.
pub struct UserTerm {
    tty: File,
    original_termios: Option<termios>,
}

impl UserTerm {
    /// Open the user's terminal.
    pub fn open() -> io::Result<Self> {
        Ok(Self {
            tty: OpenOptions::new().read(true).write(true).open("/dev/tty")?,
            original_termios: None,
        })
    }

    pub(crate) fn get_size(&self) -> io::Result<TermSize> {
        let mut term_size = MaybeUninit::<TermSize>::uninit();

        // SAFETY: This passes a valid file descriptor and valid pointer (of
        // the correct type) to the TIOCGWINSZ ioctl; see:
        // https://man7.org/linux/man-pages/man2/TIOCGWINSZ.2const.html
        cerr(unsafe {
            ioctl(
                self.tty.as_raw_fd(),
                TIOCGWINSZ,
                term_size.as_mut_ptr().cast::<winsize>(),
            )
        })?;

        // SAFETY: if we arrived at this point, `term_size` was initialized.
        Ok(unsafe { term_size.assume_init() })
    }

    /// Copy the settings of the user's terminal to the `dst` terminal.
    pub fn copy_to<D: AsFd>(&self, dst: &D) -> io::Result<()> {
        let src = self.tty.as_raw_fd();
        let dst = dst.as_fd().as_raw_fd();

        // SAFETY: tt_src and tt_dst will be initialized by `tcgetattr`.
        let (tt_src, mut tt_dst) = unsafe {
            let mut tt_src = MaybeUninit::<termios>::uninit();
            let mut tt_dst = MaybeUninit::<termios>::uninit();

            cerr(tcgetattr(src, tt_src.as_mut_ptr()))?;
            cerr(tcgetattr(dst, tt_dst.as_mut_ptr()))?;

            (tt_src.assume_init(), tt_dst.assume_init())
        };

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
        //
        // SAFETY: the cfXXXXspeed calls are passed valid pointers and
        // cannot cause UB even if the speed would be incorrect.
        unsafe {
            let mut speed = cfgetospeed(&tt_src);
            // Zero output speed closes the connection.
            if speed == libc::B0 {
                speed = libc::B38400;
            }
            cfsetospeed(&mut tt_dst, speed);

            speed = cfgetispeed(&tt_src);
            cfsetispeed(&mut tt_dst, speed);
        }

        // SAFETY: dst is a valid file descriptor and `tt_dst` is an
        // initialized struct obtained through tcgetattr; so this is safe to
        // pass to `tcsetattr`.
        unsafe { tcsetattr_nobg(dst, TCSAFLUSH, &tt_dst) }?;

        let mut wsize = MaybeUninit::<winsize>::uninit();
        // SAFETY: TIOCGWINSZ ioctl expects one argument of type *mut winsize
        cerr(unsafe { ioctl(src, TIOCGWINSZ, wsize.as_mut_ptr()) })?;
        // SAFETY: wsize has been initialized by the TIOCGWINSZ ioctl
        cerr(unsafe { ioctl(dst, TIOCSWINSZ, wsize.as_ptr()) })?;

        Ok(())
    }

    /// Set the user's terminal to raw mode. Enable terminal signals if `with_signals` is set to
    /// `true`.
    pub fn set_raw_mode(&mut self, with_signals: bool, preserve_oflag: bool) -> io::Result<()> {
        let fd = self.tty.as_raw_fd();

        // Retrieve the original terminal (if we haven't done so already)
        let mut term = if let Some(termios) = self.original_termios {
            termios
        } else {
            // SAFETY: `termios` is a valid pointer to pass to tcgetattr; if that calls succeeds,
            // it will have initialized the `termios` structure
            *self.original_termios.insert(unsafe {
                let mut termios = MaybeUninit::uninit();
                cerr(tcgetattr(fd, termios.as_mut_ptr()))?;
                termios.assume_init()
            })
        };

        // Set terminal to raw mode.
        let oflag = term.c_oflag;
        // SAFETY: `term` is a valid, initialized struct of type `termios`, which
        // was previously obtained through `tcgetattr`.
        unsafe { cfmakeraw(&mut term) };
        if preserve_oflag {
            term.c_oflag = oflag;
        }
        // Enable terminal signals.
        if with_signals {
            term.c_cflag |= ISIG;
        }

        // SAFETY: `fd` is a valid file descriptor for the tty; for `term`: same as above.
        unsafe { tcsetattr_nobg(fd, TCSADRAIN, &term) }?;

        Ok(())
    }

    /// Restore the saved terminal settings if we are in the foreground process group.
    ///
    /// This change is done after waiting for all the queued output to be written. To discard the
    /// queued input `flush` must be set to `true`.
    pub fn restore(&mut self, flush: bool) -> io::Result<()> {
        if let Some(termios) = self.original_termios.take() {
            let fd = self.tty.as_raw_fd();
            let flags = if flush { TCSAFLUSH } else { TCSADRAIN };
            // SAFETY: `fd` is a valid file descriptor for the tty; and `termios` is a valid pointer
            // that was obtained through `tcgetattr`.
            unsafe { tcsetattr_nobg(fd, flags, &termios) }?;
        }

        Ok(())
    }

    /// This is like `tcsetpgrp` but it only suceeds if we are in the foreground process group.
    pub fn tcsetpgrp_nobg(&self, pgrp: ProcessId) -> io::Result<()> {
        // This function is based around the fact that we receive `SIGTTOU` if we call `tcsetpgrp` and
        // we are not in the foreground process group.

        catching_sigttou(|| self.tcsetpgrp(pgrp))
    }
}

impl AsFd for UserTerm {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.tty.as_fd()
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
