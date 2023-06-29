mod backchannel;
mod monitor;
mod parent;
mod pipe;

use std::ffi::c_int;

pub(super) use parent::exec_pty;

use crate::system::signal::SignalNumber;

/// Continue running in the foreground
pub(super) const SIGCONT_FG: SignalNumber = -2;
/// Continue running in the background
pub(super) const SIGCONT_BG: SignalNumber = -3;

enum CommandStatus {
    Exit(c_int),
    Term(SignalNumber),
    Stop(SignalNumber),
}
