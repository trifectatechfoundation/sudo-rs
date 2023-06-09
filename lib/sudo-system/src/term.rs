use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    ptr::null_mut,
};

use sudo_cutils::cerr;

use crate::interface::ProcessId;

pub fn openpty() -> io::Result<(OwnedFd, OwnedFd)> {
    let (mut leader, mut follower) = (0, 0);
    cerr(unsafe {
        libc::openpty(
            &mut leader,
            &mut follower,
            null_mut::<libc::c_char>(),
            null_mut::<libc::termios>(),
            null_mut::<libc::winsize>(),
        )
    })?;

    Ok(unsafe { (OwnedFd::from_raw_fd(leader), OwnedFd::from_raw_fd(follower)) })
}

pub fn set_controlling_terminal<F: AsRawFd>(fd: &F) -> io::Result<()> {
    cerr(unsafe { libc::ioctl(fd.as_raw_fd(), libc::TIOCSCTTY, 0) })?;
    Ok(())
}

/// Set the foreground process group ID associated with the `fd` terminal device to `pgrp`.
pub fn tcsetpgrp<F: AsRawFd>(fd: &F, pgrp: ProcessId) -> io::Result<()> {
    cerr(unsafe { libc::tcsetpgrp(fd.as_raw_fd(), pgrp) }).map(|_| ())
}

/// Get the foreground process group ID associated with the `fd` terminal device.
pub fn tcgetpgrp<F: AsRawFd>(fd: &F) -> io::Result<ProcessId> {
    cerr(unsafe { libc::tcgetpgrp(fd.as_raw_fd()) })
}

#[cfg(test)]
mod tests {
    use libc::SIGHUP;

    use crate::{
        getpgid,
        interface::ProcessId,
        setsid,
        signal::{SignalAction, SignalHandler},
        term::*,
    };

    #[test]
    fn tcgetpgrp_works() {
        let stdout = std::io::stdout();
        let pgrp = getpgid(std::process::id() as ProcessId).unwrap();
        assert_eq!(tcgetpgrp(&stdout).unwrap(), pgrp);
    }

    #[test]
    fn tcsetpgrp_works() {
        // Ignore `SIGHUP` to avoid hanging up when we change the controlling terminal.
        let _handler = SignalHandler::with_action(SIGHUP, SignalAction::Ignore).unwrap();
        // Open a new pseudoterminal.
        let (leader, _follower) = openpty().unwrap();
        // The pty leader should not have a foreground process group yet.
        assert_eq!(tcgetpgrp(&leader).unwrap(), 0);
        // Create a new session so we can change the controlling terminal.
        setsid().unwrap();
        // Set the pty leader as the controlling terminal. 
        set_controlling_terminal(&leader).unwrap();
        // Set us as the foreground process group of the pty leader.
        let pgid = getpgid(0).unwrap();
        tcsetpgrp(&leader, pgid).unwrap();
        // Check that we are in fact the foreground process group of the pty leader.
        assert_eq!(pgid, tcgetpgrp(&leader).unwrap());
    }
}
