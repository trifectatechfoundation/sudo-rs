use std::io;

use crate::log::dev_warn;

use super::{consts::*, set::SignalAction, signal_name, SignalNumber};

/// A handler for a signal.
///
/// When a value of this type is dropped, it will try to restore the action that was registered for
/// the signal prior to calling [`SignalHandler::register`].
pub(crate) struct SignalHandler {
    signal: SignalNumber,
    original_action: SignalAction,
}

impl SignalHandler {
    const FORBIDDEN: &[SignalNumber] = &[SIGKILL, SIGSTOP];

    /// Register a new handler for the given signal with the provided behavior.
    ///
    /// # Panics
    ///
    /// If it is not possible to override the action for the provided signal.
    pub(crate) fn register(
        signal: SignalNumber,
        behavior: SignalHandlerBehavior,
    ) -> io::Result<Self> {
        if Self::FORBIDDEN.contains(&signal) {
            panic!(
                "the {} signal action cannot be overriden",
                signal_name(signal)
            );
        }

        let action = SignalAction::new(behavior)?;
        let original_action = action.register(signal)?;

        Ok(Self {
            signal,
            original_action,
        })
    }

    /// Forget this signal handler.
    ///
    /// This can be used to avoid restoring the original action for the signal.
    pub(crate) fn forget(self) {
        std::mem::forget(self)
    }
}

impl Drop for SignalHandler {
    #[track_caller]
    fn drop(&mut self) {
        let signal = self.signal;
        if let Err(err) = self.original_action.register(signal) {
            dev_warn!(
                "cannot restore original action for {}: {err}",
                signal_name(signal),
            )
        }
    }
}

/// The possible behaviors for a [`SignalHandler`].
pub(crate) enum SignalHandlerBehavior {
    /// Execute the default action for the signal.
    Default,
    /// Ignore the arrival of the signal.
    Ignore,
    /// Stream the signal information into the latest initialized instance of [`super::SignalStream`].
    Stream,
}
