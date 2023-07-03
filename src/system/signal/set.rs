use crate::cutils::cerr;

use super::{handler::SignalHandlerBehavior, SignalNumber};

use std::{io, mem::MaybeUninit};

#[repr(transparent)]
pub(super) struct SignalAction {
    raw: libc::sigaction,
}

impl SignalAction {
    pub(super) fn new(behavior: SignalHandlerBehavior) -> io::Result<Self> {
        let sa_mask = SignalSet::full()?;
        let mut sa_flags = libc::SA_RESTART;

        let sa_sigaction = match behavior {
            SignalHandlerBehavior::Default => libc::SIG_DFL,
            SignalHandlerBehavior::Ignore => libc::SIG_IGN,
            SignalHandlerBehavior::Stream => {
                sa_flags |= libc::SA_SIGINFO;
                super::stream::send_siginfo as libc::sighandler_t
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

    pub(super) fn register(&self, signal: SignalNumber) -> io::Result<Self> {
        let mut original_action = MaybeUninit::<Self>::zeroed();

        cerr(unsafe {
            libc::sigaction(signal, &self.raw, original_action.as_mut_ptr().cast())
        })?;

        Ok(unsafe { original_action.assume_init() })
    }
}

#[repr(transparent)]
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

    fn sigprocmask(&self, how: libc::c_int) -> io::Result<Self> {
        let mut original_set = MaybeUninit::<Self>::zeroed();

        cerr(unsafe { libc::sigprocmask(how, &self.raw, original_set.as_mut_ptr().cast()) })?;

        Ok(unsafe { original_set.assume_init() })
    }

    pub(crate) fn block(&self) -> io::Result<Self> {
        self.sigprocmask(libc::SIG_BLOCK)
    }

    pub(crate) fn set_mask(&self) -> io::Result<Self> {
        self.sigprocmask(libc::SIG_SETMASK)
    }
}

