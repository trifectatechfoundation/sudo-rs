use crate::{cutils::cerr, system::make_zeroed_sigaction};

use super::{handler::SignalHandlerBehavior, SignalNumber};

use std::{io, mem::MaybeUninit};

#[repr(transparent)]
pub(super) struct SignalAction {
    raw: libc::sigaction,
}

impl SignalAction {
    pub(super) fn new(behavior: SignalHandlerBehavior) -> io::Result<Self> {
        // This guarantees that functions won't be interrupted by this signal as long as the
        // handler is alive.
        let mut sa_flags = libc::SA_RESTART;

        // We only need a full `sa_mask` if we are going to stream the signal information as we
        // don't want to be interrupted by any signals while executing `send_siginfo`.
        let (sa_sigaction, sa_mask) = match behavior {
            SignalHandlerBehavior::Default => (libc::SIG_DFL, SignalSet::empty()?),
            SignalHandlerBehavior::Ignore => (libc::SIG_IGN, SignalSet::empty()?),
            SignalHandlerBehavior::Stream => {
                // Specify that we want to pass a signal-catching function in `sa_sigaction`.
                sa_flags |= libc::SA_SIGINFO;
                (
                    super::stream::send_siginfo as libc::sighandler_t,
                    SignalSet::full()?,
                )
            }
        };

        let mut raw: libc::sigaction = make_zeroed_sigaction();
        raw.sa_sigaction = sa_sigaction;
        raw.sa_mask = sa_mask.raw;
        raw.sa_flags = sa_flags;
        raw.sa_restorer = None;

        Ok(Self { raw })
    }

    pub(super) fn register(&self, signal: SignalNumber) -> io::Result<Self> {
        let mut original_action = MaybeUninit::<Self>::zeroed();

        cerr(unsafe { libc::sigaction(signal, &self.raw, original_action.as_mut_ptr().cast()) })?;

        Ok(unsafe { original_action.assume_init() })
    }
}

// A signal set that can be used to mask signals.
#[repr(transparent)]
pub(crate) struct SignalSet {
    raw: libc::sigset_t,
}

impl SignalSet {
    /// Create an empty set.
    pub(crate) fn empty() -> io::Result<Self> {
        let mut set = MaybeUninit::<Self>::zeroed();

        cerr(unsafe { libc::sigemptyset(set.as_mut_ptr().cast()) })?;

        Ok(unsafe { set.assume_init() })
    }

    /// Create a set containing all the signals.
    pub(crate) fn full() -> io::Result<Self> {
        let mut set = MaybeUninit::<Self>::zeroed();

        cerr(unsafe { libc::sigfillset(set.as_mut_ptr().cast()) })?;

        Ok(unsafe { set.assume_init() })
    }

    fn sigprocmask(&self, how: libc::c_int) -> io::Result<Self> {
        let mut original_set = MaybeUninit::<Self>::zeroed();

        cerr(unsafe { libc::sigprocmask(how, &self.raw, original_set.as_mut_ptr().cast()) })?;

        Ok(unsafe { original_set.assume_init() })
    }

    /// Block all the signals in this set and return the previous set of blocked signals.
    ///
    /// After calling this function successfully, the set of blocked signals will be the union of
    /// the previous set of blocked signals and this set.
    pub(crate) fn block(&self) -> io::Result<Self> {
        self.sigprocmask(libc::SIG_BLOCK)
    }

    /// Block only the signals that are in this set and return the previous set of blocked signals.
    ///
    /// After calling this function successfully, the set of blocked signals will be the exactly
    /// this set.
    pub(crate) fn set_mask(&self) -> io::Result<Self> {
        self.sigprocmask(libc::SIG_SETMASK)
    }
}
