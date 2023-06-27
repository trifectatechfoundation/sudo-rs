use std::io;

use crate::system::signal::{SignalHandler, SignalNumber};

use signal_hook::consts::*;

pub(super) struct SignalManager {
    handlers: [SignalHandler; Signal::ALL.len()],
}

impl SignalManager {
    /// Unregister all the handlers created by the dispatcher.
    pub(super) fn unregister_handlers(self) {
        for handler in self.handlers {
            handler.unregister();
        }
    }

    pub(super) fn handlers(&self) -> impl Iterator<Item = (Signal, &SignalHandler)> {
        (Signal::ALL).iter().copied().zip(self.handlers.iter())
    }

}

macro_rules! define_signals {
    ($($signal:ident = $index:literal,)*) => {
        #[allow(clippy::upper_case_acronyms)]
        /// Signals that can be handled
        #[derive(Clone, Copy, Debug)]
        pub(super) enum Signal {
            $($signal,)*
        }

        impl Signal {
            const ALL: &[Self] = &[$(Self::$signal,)*];

            pub(super) fn try_from_number(signal: SignalNumber) -> Option<Self> {
                match signal {
                    $($signal => Some(Self::$signal),)*
                    _ => None,
                }
            }
        }

        impl SignalManager {
            pub(super) fn new() -> io::Result<Self> {
                Ok(Self {
                    handlers: [$(SignalHandler::new($signal)?,)*],
                })
            }

            pub(super) fn get_handler(&self, signal: Signal) -> &SignalHandler {
                match signal {
                    $(Signal::$signal => &self.handlers[$index],)*
                }
            }

            pub(super) fn get_handler_mut(&mut self, signal: Signal) -> &mut SignalHandler {
                match signal {
                    $(Signal::$signal => &mut self.handlers[$index],)*
                }
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
}
