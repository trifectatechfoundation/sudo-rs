//! Utilities to handle signals.
mod handler;
mod info;
mod set;
mod stream;

pub(crate) use handler::{SignalHandler, SignalHandlerBehavior};
pub(crate) use set::SignalSet;
pub(crate) use stream::{register_handlers, SignalStream};

use std::borrow::Cow;

pub(crate) type SignalNumber = libc::c_int;

macro_rules! define_consts {
    ($($signal:ident,)*) => {
        pub(crate) mod consts {
            pub(crate) use libc::{$($signal,)*};
        }

        pub(crate) fn signal_name(signal: SignalNumber) -> Cow<'static, str> {
            match signal {
                $(consts::$signal => stringify!($signal).into(),)*
                _ => format!("unknown signal ({signal})").into(),
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
