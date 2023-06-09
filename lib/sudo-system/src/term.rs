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
    use crate::{getpgid, interface::ProcessId, term::*};

    #[test]
    fn tcgetpgrp_matches_getpgid() {
        let stdout = std::io::stdout();
        let pgrp = getpgid(std::process::id() as ProcessId).unwrap();
        assert_eq!(tcgetpgrp(&stdout).unwrap(), pgrp);
    }
}
