//! Utilities to handle signals.
#![warn(unused)]
use std::{
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
};

use crate::cutils::cerr;
use libc::{c_int, c_void, siginfo_t, MSG_DONTWAIT};

use super::interface::ProcessId;

const SIGINFO_SIZE: usize = std::mem::size_of::<siginfo_t>();

pub(crate) type SignalNumber = c_int;

/// Information related to the arrival of a signal.
pub(crate) struct SignalInfo {
    info: siginfo_t,
}

impl SignalInfo {
    /// Returns whether the signal was sent by the user or not.
    pub(crate) fn is_user_signaled(&self) -> bool {
        // FIXME: we should check if si_code is equal to SI_USER but for some reason the latter it
        // is not available in libc.
        self.info.si_code <= 0
    }

    /// Gets the PID that sent the signal.
    pub(crate) fn pid(&self) -> ProcessId {
        // FIXME: some signals don't set si_pid.
        unsafe { self.info.si_pid() }
    }

    /// Gets the signal number.
    pub(crate) fn signal(&self) -> SignalNumber {
        self.info.si_signo
    }
}

static mut TX: RawFd = -1;

fn send(_signal: SignalNumber, info: &siginfo_t, _context: *const c_void) {
    let tx = unsafe { TX };
    if tx != -1 {
        unsafe {
            libc::send(
                tx,
                (info as *const siginfo_t).cast(),
                SIGINFO_SIZE,
                MSG_DONTWAIT,
            );
        }
    }
}

pub(crate) struct SignalStream {
    rx: UnixStream,
    _tx: UnixStream,
}

impl SignalStream {
    pub(crate) fn new() -> io::Result<Self> {
        let (rx, tx) = UnixStream::pair()?;

        unsafe { TX = tx.as_raw_fd() };

        Ok(Self { rx, _tx: tx })
    }
    /// Receives the information related to the arrival of a signal.
    ///
    /// Calling this function will block until a signal whose action is set to
    /// [`SignalAction::stream`] arrives. Otherwise it will block indefinitely.
    ///
    /// Note that calling this function will only retrieve the information related to a single
    /// signal arrival even if several signals have been sent to the process.
    pub(crate) fn recv(&mut self) -> io::Result<SignalInfo> {
        let mut info = MaybeUninit::<siginfo_t>::uninit();
        let fd = self.rx.as_raw_fd();
        let bytes = cerr(unsafe { libc::recv(fd, info.as_mut_ptr().cast(), SIGINFO_SIZE, 0) })?;

        if bytes as usize != SIGINFO_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Not enough bytes when receiving `siginfo_t`",
            ));
        }
        // SAFETY: we can assume `info` is initialized because `recv` wrote enough bytes to fill
        // the value and `siginfo_t` is POD.
        let info = unsafe { info.assume_init() };
        Ok(SignalInfo { info })
    }
}

impl AsRawFd for SignalStream {
    fn as_raw_fd(&self) -> RawFd {
        self.rx.as_raw_fd()
    }
}

pub(crate) enum SignalHandlerBehavior {
    Default,
    Ignore,
    Stream,
}

pub(crate) struct SignalAction {
    raw: libc::sigaction,
}

impl SignalAction {
    fn new(behavior: SignalHandlerBehavior) -> io::Result<Self> {
        let sa_mask = SignalSet::full()?;
        let mut sa_flags = libc::SA_RESTART;

        let sa_sigaction = match behavior {
            SignalHandlerBehavior::Default => libc::SIG_DFL,
            SignalHandlerBehavior::Ignore => libc::SIG_IGN,
            SignalHandlerBehavior::Stream => {
                sa_flags |= libc::SA_SIGINFO;
                send as libc::sighandler_t
            }
        };

        Ok(Self {
            raw: libc::sigaction {
                sa_sigaction,
                sa_mask: sa_mask.raw,
                sa_flags,
                sa_restorer: None,
            },
        })
    }

    fn register(&self, signal: SignalNumber) -> io::Result<Self> {
        let mut original_action = MaybeUninit::<libc::sigaction>::zeroed();

        cerr(unsafe { libc::sigaction(signal, &self.raw, original_action.as_mut_ptr()) })?;

        Ok(Self {
            raw: unsafe { original_action.assume_init() },
        })
    }
}

pub(crate) struct SignalHandler {
    signal: SignalNumber,
    original_action: SignalAction,
}

impl SignalHandler {
    pub(crate) fn new(signal: SignalNumber, behavior: SignalHandlerBehavior) -> io::Result<Self> {
        let action = SignalAction::new(behavior)?;
        let original_action = action.register(signal)?;

        Ok(Self {
            signal,
            original_action,
        })
    }

    pub(crate) fn forget(self) {
        std::mem::forget(self)
    }
}

impl Drop for SignalHandler {
    fn drop(&mut self) {
        self.original_action.register(self.signal).ok();
    }
}

pub(crate) struct SignalSet {
    raw: libc::sigset_t,
}

impl SignalSet {
    pub(crate) fn full() -> io::Result<Self> {
        let mut raw = MaybeUninit::<libc::sigset_t>::uninit();

        cerr(unsafe { libc::sigfillset(raw.as_mut_ptr()) })?;

        Ok(Self {
            raw: unsafe { raw.assume_init() },
        })
    }

    fn sigprocmask(&self, how: c_int) -> io::Result<Self> {
        let mut original_set = MaybeUninit::<libc::sigset_t>::zeroed();

        cerr(unsafe { libc::sigprocmask(how, &self.raw, original_set.as_mut_ptr()) })?;

        Ok(Self {
            raw: unsafe { original_set.assume_init() },
        })
    }

    pub(crate) fn block(&self) -> io::Result<Self> {
        self.sigprocmask(libc::SIG_BLOCK)
    }

    pub(crate) fn set_mask(&self) -> io::Result<Self> {
        self.sigprocmask(libc::SIG_SETMASK)
    }
}

macro_rules! define_consts {
    ($($signal:ident,)*) => {
        pub(crate) mod consts {
            pub(crate) use libc::{$($signal,)*};
        }

        pub(crate) fn signal_name(signal: SignalNumber) -> Option<&'static str> {
            match signal {
                $(consts::$signal => Some(stringify!($signal)),)*
                _ => None,
            }
        }
    };
}

define_consts! {
    SIGINT,
    SIGQUIT,
    SIGTSTP,
    SIGTERM,
    SIGHUP,
    SIGALRM,
    SIGPIPE,
    SIGUSR1,
    SIGUSR2,
    SIGCHLD,
    SIGCONT,
    SIGWINCH,
    SIGTTIN,
    SIGTTOU,
    SIGKILL,
    SIGSTOP,
}
