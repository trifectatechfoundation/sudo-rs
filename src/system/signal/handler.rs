use std::io;

use super::{set::SignalAction, SignalNumber};

pub(crate) struct SignalHandler {
    signal: SignalNumber,
    original_action: SignalAction,
}

impl SignalHandler {
    pub(crate) fn new(
        signal: SignalNumber,
        behavior: SignalHandlerBehavior,
    ) -> io::Result<Self> {
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

pub(crate) enum SignalHandlerBehavior {
    Default,
    Ignore,
    Stream,
}

