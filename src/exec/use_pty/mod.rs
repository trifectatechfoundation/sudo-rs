mod backchannel;
mod monitor;
mod parent;
mod pipe;

pub(super) use parent::exec_pty;

use crate::system::signal::SignalNumber;

/// Continue running in the foreground
pub(super) const SIGCONT_FG: SignalNumber = -2;
/// Continue running in the background
pub(super) const SIGCONT_BG: SignalNumber = -3;
