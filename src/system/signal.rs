//! Utilities to handle signals.

// FIXME: It should be possible to implement the same functionality without `signal_hook` and
// `signal_hook_registry` without much effort. But given that async-signal-safety can be tricky we
// should keep using those crates unless those dependencies become a concern.
use std::{
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

use crate::cutils::cerr;
use libc::{c_int, siginfo_t, MSG_DONTWAIT};
use signal_hook::consts::*;
use signal_hook::low_level::{emulate_default_handler, signal_name};
use signal_hook_registry::{register_sigaction, unregister, SigId, FORBIDDEN};

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

/// The action to be executed by a [`SignalHandler`] when a signal arrives.
#[repr(u8)]
pub(crate) enum SignalAction {
    /// Stream the signal information so it can be received using [`SignalHandler::recv`].
    Stream = 0,
    /// Emulate the default handler for the signal (e.g. terminating the process when receiving `SIGTERM` or
    /// stopping the process when receiving `SIGTSTP`).
    Default = 1,
    /// Ignore the incoming signal.
    Ignore = 2,
}

impl SignalAction {
    fn try_new(val: u8) -> Option<Self> {
        if val == Self::Stream as u8 {
            Some(Self::Stream)
        } else if val == Self::Default as u8 {
            Some(Self::Default)
        } else if val == Self::Ignore as u8 {
            Some(Self::Ignore)
        } else {
            None
        }
    }
}

/// A type that handles the received signals according to different actions that can be configured
/// using [`SignalHandler::set_action`] and [`SignalHandler::with_actions`].
pub(crate) struct SignalHandler {
    /// The reading side of the self-pipe.
    ///
    /// It is used to communicate that a signal was received if the set action is
    /// [`SignalAction::Stream`].
    rx: UnixStream,
    /// The writing side of the self-pipe.
    ///
    /// It is used so the socket is closed when dropping the handler.
    tx: UnixStream,
    /// The current actions to be executed when each signal arrives.
    ///
    /// The atomic integer used here must match the representation type of [`SignalAction`].
    actions: [Arc<AtomicU8>; Signal::ALL.len()],
    /// The identifier under which the action for a signal was registered.
    ///
    /// It can be used to unregister or re-register the action if needed.
    sig_ids: [SigId; Signal::ALL.len()],
}

fn send(tx: RawFd, info: &siginfo_t) {
    unsafe {
        libc::send(
            tx,
            (info as *const siginfo_t).cast(),
            SIGINFO_SIZE,
            MSG_DONTWAIT,
        );
    }
}

macro_rules! define_signals {
    ($($signal:ident = $index:literal,)*) => {
        #[allow(clippy::upper_case_acronyms)]
        /// Signals that can be handled by [`SignalHandler`]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum Signal {
            $($signal,)*
        }

        impl Signal {
            pub(crate) const ALL: &[Self] = &[$(Self::$signal,)*];

            /// Get the number for this signal.
            pub(crate) fn number(self) -> SignalNumber {
                match self {
                    $(Self::$signal => $signal,)*
                }
            }

            /// Create a new [`Signal`] from its number.
            pub(crate) fn from_number(number: SignalNumber) -> Option<Self> {
                match number {
                    $($signal => Some(Self::$signal),)*
                    _ => None,
                }
            }
        }

        impl SignalInfo {
            /// Gets the signal number.
            pub(crate) fn signal(&self) -> Signal {
                match self.info.si_signo {
                    $($signal => Signal::$signal,)*
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
            pub(crate) fn with_actions<F: Fn(Signal) -> SignalAction>(f: F) -> io::Result<Self> {
                let (rx, tx) = UnixStream::pair()?;

                let actions = [$(Arc::new(AtomicU8::from(f(Signal::$signal) as u8)),)*];

                let sig_ids = [$({
                    let action = Arc::clone(&actions[$index]);
                    let tx = tx.as_raw_fd();
                    // SAFETY: The closure passed to `register_sigaction` is run inside a signal handler,
                    //
                    // meaning that all the functions called inside it must be async-signal-safe as defined
                    // by POSIX. This code should be sound because:
                    //
                    // - This function does not panic.
                    // - The `action` atomic value is lock-free.
                    // - The `send` function only calls the `send` syscall which is async-signal-safe.
                    // - The `emulate_default_handler` function is async-signal-safe according to
                    // `signal_hook`.
                    unsafe {
                        register_sigaction($signal, move |info| {
                            if let Some(action) = SignalAction::try_new(action.load(Ordering::SeqCst)) {
                                match action {
                                    SignalAction::Stream => send(tx, info),
                                    SignalAction::Default => {
                                        emulate_default_handler($signal).ok();
                                    }
                                    SignalAction::Ignore => {}
                                }
                            }
                        })
                    }?
                },)*];

                Ok(Self { rx, tx, actions, sig_ids })
            }

            /// Changes the signal action for this handler and returns the previously set action.
            pub(crate) fn set_action(&self, signal: Signal, action: SignalAction) -> SignalAction {
                let current_action = match signal {
                    $(Signal::$signal => &self.actions[$index],)*
                };

                SignalAction::try_new(current_action.swap(action as u8, Ordering::SeqCst))
                    .unwrap_or(SignalAction::Ignore)
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

/// Set the action to [`SignalAction::Default`] when dropping.
impl Drop for SignalHandler {
    fn drop(&mut self) {
        for &signal in Signal::ALL {
            self.set_action(signal, SignalAction::Default);
        }
    }
}

impl AsRawFd for SignalHandler {
    fn as_raw_fd(&self) -> RawFd {
        self.rx.as_raw_fd()
    }
}

impl SignalHandler {
    /// Creates a new signal handler that executes the [`SignalAction::Stream`] action.
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
    pub(crate) fn new() -> io::Result<Self> {
        Self::with_actions(|_| SignalAction::Stream)
    }

    /// Receives the information related to the arrival of a signal.
    ///
    /// Calling this function will block until a signal whose action is set to
    /// [`SignalAction::Stream`] arrives. Otherwise it will block indefinitely.
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

    /// Unregister the handler.
    ///
    /// This leaves the current process without a handler for the signals handled by this handler.
    /// Meaning that the process will ignore the signal when receiving it.
    pub(crate) fn unregister(&self) {
        for &sig_id in &self.sig_ids {
            unregister(sig_id);
        }
    }
}
