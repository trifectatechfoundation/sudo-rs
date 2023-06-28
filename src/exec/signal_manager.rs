use std::io;

use crate::system::{
    poll::PollEvent,
    signal::{SignalAction, SignalHandler, SignalInfo, SignalNumber},
};

use signal_hook::consts::*;

use super::event::{EventRegistry, Process};

pub(super) struct SignalManager {
    handlers: [SignalHandler; Signal::ALL.len()],
}

impl SignalManager {
    /// Unregister all the handlers created by the registry.
    pub(super) fn unregister_handlers(self) {
        for handler in self.handlers {
            handler.unregister();
        }
    }

    pub(super) fn register_handlers<T: Process>(
        &self,
        registry: &mut EventRegistry<T>,
        f: fn(Signal) -> T::Event,
    ) {
        for (&signal, handler) in Signal::ALL.iter().zip(&self.handlers) {
            registry.register_event(handler, PollEvent::Readable, |_| f(signal));
        }
    }
}

macro_rules! define_signals {
    ($($signal:ident = $index:literal,)*) => {
        #[allow(clippy::upper_case_acronyms)]
        /// Signals that can be handled
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

            pub(super) fn set_action(&self, signal: Signal, action: SignalAction) -> SignalAction {
                let handler = match signal {
                    $(Signal::$signal => &self.handlers[$index],)*
                };

                handler.set_action(action)
            }

            pub(super) fn recv(&mut self, signal: Signal) -> io::Result<SignalInfo> {
                let handler = match signal {
                    $(Signal::$signal => &mut self.handlers[$index],)*
                };

                handler.recv()
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
