use std::fmt;

use crate::system::interface::ProcessId;

use super::SignalNumber;

/// Information related to the arrival of a signal.
#[repr(transparent)]
pub(crate) struct SignalInfo {
    info: libc::siginfo_t,
}

impl SignalInfo {
    pub(super) const SIZE: usize = std::mem::size_of::<Self>();

    /// Returns whether the signal was sent by the user or not.
    fn is_user_signaled(&self) -> bool {
        // This matches the definition of the SI_FROMUSER macro.
        self.info.si_code <= 0
    }

    /// Gets the PID that sent the signal.
    pub(crate) fn signaler_pid(&self) -> Option<ProcessId> {
        if self.is_user_signaled() {
            // SAFETY: si_pid is always initialized if the signal is user signaled.
            unsafe { Some(self.info.si_pid()) }
        } else {
            None
        }
    }

    /// Gets the signal number.
    pub(crate) fn signal(&self) -> SignalNumber {
        self.info.si_signo
    }
}

impl fmt::Display for SignalInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} from ",
            if self.is_user_signaled() {
                " user signaled"
            } else {
                ""
            },
            self.signal(),
        )?;
        if let Some(pid) = self.signaler_pid() {
            write!(f, "{pid}")
        } else {
            write!(f, "<none>")
        }
    }
}
