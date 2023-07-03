//! Utilities to handle signals.
use std::{
    borrow::Cow,
    cell::OnceCell,
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, OnceLock,
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
}

static mut TX: RawFd = -1;

fn send(_signal: SignalNumber, info: &siginfo_t, _context: *const c_void) {
    let tx = unsafe { TX };
    unsafe {
        libc::send(
            tx,
            (info as *const siginfo_t).cast(),
            SIGINFO_SIZE,
            MSG_DONTWAIT,
        );
    }
}

/// A type that handles the received signals according to different actions that can be configured
/// using [`SignalHandler::set_action`] and [`SignalHandler::with_actions`].
pub(crate) struct SignalHandler {
    /// The reading side of the self-pipe.
    ///
    /// It is used to communicate that a signal was received if the set action is
    /// [`SignalAction::stream`].
    rx: UnixStream,
    /// The writing side of the self-pipe.
    ///
    /// It is used so the socket is closed when dropping the handler.
    tx: UnixStream,

    original_actions: [SignalAction; Signal::ALL.len()],
}

#[repr(transparent)]
struct SignalMask {
    raw: libc::sigset_t,
}

impl SignalMask {
    fn new() -> io::Result<Self> {
        let mut raw = MaybeUninit::<libc::sigset_t>::uninit();

        cerr(unsafe { libc::sigemptyset(raw.as_mut_ptr()) })?;

        Ok(Self {
            raw: unsafe { raw.assume_init() },
        })
    }

    fn full() -> io::Result<Self> {
        let mut raw = MaybeUninit::<libc::sigset_t>::uninit();

        cerr(unsafe { libc::sigfillset(raw.as_mut_ptr()) })?;

        Ok(Self {
            raw: unsafe { raw.assume_init() },
        })
    }

    fn insert(&mut self, signal: SignalNumber) -> io::Result<()> {
        cerr(unsafe { libc::sigaddset(&mut self.raw, signal) })?;

        Ok(())
    }

    fn remove(&mut self, signal: SignalNumber) -> io::Result<()> {
        cerr(unsafe { libc::sigdelset(&mut self.raw, signal) })?;

        Ok(())
    }

    fn contains(&self, signal: SignalNumber) -> io::Result<bool> {
        cerr(unsafe { libc::sigismember(&self.raw, signal) }).map(|res| res == 1)
    }
}

enum SignalActionHandler {
    Default,
    Ignore,
    Stream,
}

pub(crate) struct SignalAction {
    raw: libc::sigaction,
}

impl SignalAction {
    pub(crate) fn default() -> io::Result<Self> {
        Self::new(SignalActionHandler::Default)
    }

    pub(crate) fn ignore() -> io::Result<Self> {
        Self::new(SignalActionHandler::Ignore)
    }

    pub(crate) fn stream() -> io::Result<Self> {
        Self::new(SignalActionHandler::Stream)
    }

    fn new(action_handler: SignalActionHandler) -> io::Result<Self> {
        let sa_mask = SignalMask::full()?;
        let mut sa_flags = libc::SA_RESTART;

        let sa_sigaction = match action_handler {
            SignalActionHandler::Default => libc::SIG_DFL,
            SignalActionHandler::Ignore => libc::SIG_IGN,
            SignalActionHandler::Stream => {
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

impl From<SignalAction> for io::Result<SignalAction> {
    fn from(action: SignalAction) -> Self {
        Ok(action)
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

macro_rules! define_signals {
    ($($signal:ident = $index:literal,)*) => {
        define_consts! { SIGKILL, SIGSTOP, $($signal,)* }

        #[allow(clippy::upper_case_acronyms)]
        /// Signals that can be handled by [`SignalHandler`]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum Signal {
            $($signal,)*
        }

        impl Signal {
            pub(crate) const ALL: &[Self] = &[$(Self::$signal,)*];

            /// Create a new [`Signal`] from its number.
            pub(crate) fn from_number(number: SignalNumber) -> Option<Self> {
                match number {
                    $(consts::$signal => Some(Self::$signal),)*
                    _ => None,
                }
            }
        }

        impl From<Signal> for SignalNumber {
            fn from(signal: Signal) -> SignalNumber {
                match signal {
                    $(Signal::$signal => consts::$signal,)*
                }
            }
        }

        impl SignalInfo {
            /// Gets the signal number.
            pub(crate) fn signal(&self) -> Signal {
                match self.info.si_signo {
                    $(consts::$signal => Signal::$signal,)*
                    _ => unreachable!(),
                }
            }
        }

        impl SignalHandler {
            /// Creates a new signal handler that executes the provided action for each signal.
            ///
            /// # Panics
            ///
            /// When `signal` is one of:
            ///
            /// * `SIGKILL`
            /// * `SIGSTOP`
            /// * `SIGILL`
            /// * `SIGFPE`
            /// * `SIGSEGV`
            pub(crate) fn with_actions<F: Fn(Signal) -> io::Result<SignalAction>>(f: F) -> io::Result<Self> {
                let (rx, tx) = UnixStream::pair()?;

                unsafe { TX = tx.as_raw_fd() };

                let original_actions = [$({ f(Signal::$signal)?.register(consts::$signal)? },)*];

                Ok(Self { rx, tx, original_actions })
            }
        }
    };
}

define_signals! {
    SIGINT = 0,
    SIGQUIT = 1,
    SIGTSTP = 2,
    SIGTERM = 3,
    SIGHUP = 4,
    SIGALRM = 5,
    SIGPIPE = 6,
    SIGUSR1 = 7,
    SIGUSR2 = 8,
    SIGCHLD = 9,
    SIGCONT = 10,
    SIGWINCH = 11,
    SIGTTIN = 12,
    SIGTTOU = 13,
}

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self, f)
    }
}

/// Restore the original signal handlers before dropping.
impl Drop for SignalHandler {
    fn drop(&mut self) {
        unsafe { TX = -1 };

        for (&signal, original_action) in Signal::ALL.into_iter().zip(self.original_actions.iter())
        {
            original_action.register(signal.into());
        }
    }
}

impl AsRawFd for SignalHandler {
    fn as_raw_fd(&self) -> RawFd {
        self.rx.as_raw_fd()
    }
}

impl SignalHandler {
    /// Creates a new signal handler that executes the [`SignalAction::stream`] action for every
    /// signal in [`Signal`].
    pub(crate) fn new() -> io::Result<Self> {
        Self::with_actions(|_| SignalAction::stream())
    }

    /// Receives the information related to the arrival of a signal.
    ///
    /// Calling this function will block until a signal whose action is set to
    /// [`SignalAction::stream`] arrives. Otherwise it will block indefinitely.
    ///
    /// Note that calling this function will only retrieve the information related to a single
    /// signal arrival even if several signals have been sent to the process.
    pub fn recv(&mut self) -> io::Result<SignalInfo> {
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

    /// Changes the signal action for this handler and returns the previously set action.
    pub(crate) fn set_action(
        &self,
        signal: Signal,
        action: impl Into<io::Result<SignalAction>>,
    ) -> io::Result<SignalAction> {
        action.into()?.register(signal.into())
    }
}

