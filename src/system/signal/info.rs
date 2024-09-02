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
    pub(crate) fn is_user_signaled(&self) -> bool {
        // FIXME: we should check if si_code is equal to SI_USER but for some reason the latter it
        // is not available in libc.
        self.info.si_code <= 0
    }

    /// Gets the PID that sent the signal.
    pub(crate) fn pid(&self) -> ProcessId {
        // FIXME: some signals don't set si_pid.
        //
        // SAFETY: this just fetches the `si_pid` field; since this is an integer,
        // even if the information is nonsense it will not cause UB. Note that
        // that a `ProcessId` does not have as type invariant that it always holds a valid
        // process id, only that it is the appropriate type for storing such ids.
        unsafe { self.info.si_pid() }
    }

    /// Gets the signal number.
    pub(crate) fn signal(&self) -> SignalNumber {
        self.info.si_signo
    }
}
