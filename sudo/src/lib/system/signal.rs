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
use signal_hook::low_level::{emulate_default_handler, signal_name};
use signal_hook_registry::{register_sigaction, unregister, SigId, FORBIDDEN};

use super::interface::ProcessId;

const SIGINFO_SIZE: usize = std::mem::size_of::<siginfo_t>();

pub type SignalNumber = c_int;

/// Information related to the arrival of a signal.
pub struct SignalInfo {
    info: siginfo_t,
}

impl SignalInfo {
    /// Returns whether the signal was sent by the user or not.
    pub fn is_user_signaled(&self) -> bool {
        // FIXME: we should check if si_code is equal to SI_USER but for some reason the latter it
        // is not available in libc.
        self.info.si_code <= 0
    }

    /// Gets the PID that sent the signal.
    pub fn pid(&self) -> ProcessId {
        // FIXME: some signals don't set si_pid.
        unsafe { self.info.si_pid() }
    }

    /// Gets the signal number.
    pub fn signal(&self) -> SignalNumber {
        self.info.si_signo
    }
}

/// The action to be executed by a [`SignalHandler`] when a signal arrives.
#[repr(u8)]
pub enum SignalAction {
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
/// using [`SignalHandler::set_action`] and [`SignalHandler::with_action`].
pub struct SignalHandler {
    /// The number of the signal being handled.
    signal: SignalNumber,
    /// The identifier under which the action of this handler was registered.
    ///
    /// It can be used to unregister or re-register the action if needed.
    sig_id: SigId,
    /// The reading side of the self-pipe.
    ///
    /// It is used to communicate that a signal was received if the set action is
    /// [`SignalAction::Stream`].
    rx: UnixStream,
    /// The current action to be executed when a signal arrives.
    ///
    /// The atomic integer used here must match the representation type of [`SignalAction`].
    action: Arc<AtomicU8>,
}

/// Set the action to [`SignalAction::Default`] when dropping.
impl Drop for SignalHandler {
    fn drop(&mut self) {
        self.set_action(SignalAction::Default);
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
    pub fn new(signal: SignalNumber) -> io::Result<Self> {
        Self::with_action(signal, SignalAction::Stream)
    }

    /// Creates a new signal handler that executes the provided action.
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
    pub fn with_action(signal: SignalNumber, action: SignalAction) -> io::Result<Self> {
        if FORBIDDEN.contains(&signal) {
            panic!(
                "SignalHandler cannot be used to handle the forbidden {} signal",
                signal_name(signal).unwrap()
            );
        }

        let (rx, tx) = UnixStream::pair()?;

        let action = Arc::new(AtomicU8::from(action as u8));

        let sig_id = {
            let action = Arc::clone(&action);
            // SAFETY: The closure passed to `register_sigaction` is run inside a signal handler,
            // meaning that all the functions called inside it must be async-signal-safe as defined
            // by POSIX. This code should be sound because:
            //
            // - This function does not panic.
            // - The `action` atomic value is lock-free.
            // - The `send` function only calls the `send` syscall which is async-signal-safe.
            // - The `emulate_default_handler` function is async-signal-safe according to
            // `signal_hook`.
            unsafe {
                register_sigaction(signal, move |info| {
                    if let Some(action) = SignalAction::try_new(action.load(Ordering::SeqCst)) {
                        match action {
                            SignalAction::Stream => send(&tx, info),
                            SignalAction::Default => {
                                emulate_default_handler(signal).ok();
                            }
                            SignalAction::Ignore => {}
                        }
                    }
                })
            }?
        };

        Ok(Self {
            signal,
            sig_id,
            rx,
            action,
        })
    }

    /// Changes the action for this handler and returns the previously set action.
    pub fn set_action(&self, action: SignalAction) -> SignalAction {
        SignalAction::try_new(self.action.swap(action as u8, Ordering::SeqCst))
            .unwrap_or(SignalAction::Ignore)
    }

    /// Receives the information related to the arrival of a signal.
    ///
    /// Calling this function will block until a signal arrives if the action for this handler is
    /// set to [`SignalAction::Stream`]. Otherwise it will block indefinitely.
    ///
    /// Note that calling this function will only retrieve the information related to a single
    /// signal arrival even if the same signal has been sent to the process more than once.
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

    /// Returns the number of the signal that is being handled.
    pub fn signal(&self) -> SignalNumber {
        self.signal
    }

    /// Unregister the handler.
    ///
    /// This leaves the current process without a handler for the signal handled by this handler.
    /// Meaning that the process will ignore the signal when receiving it.
    pub fn unregister(&self) {
        // We need to be sure that we unregister this action when the handler is dropped. If it was
        // already unregistered for whatever reason this should be a no-op.
        unregister(self.sig_id);
    }
}

fn send(tx: &UnixStream, info: &siginfo_t) {
    let fd = tx.as_raw_fd();

    unsafe {
        libc::send(
            fd,
            (info as *const siginfo_t).cast(),
            SIGINFO_SIZE,
            MSG_DONTWAIT,
        );
    }
}
