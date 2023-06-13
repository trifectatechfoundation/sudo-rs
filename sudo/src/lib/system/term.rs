use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    ptr::null_mut,
};

use crate::cutils::cerr;

use super::interface::ProcessId;

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
    use std::{
        io::{Read, Write},
        os::unix::net::UnixStream,
    };

    use crate::system::{fork, getpgid, setsid, term::*};

    #[test]
    fn tcsetpgrp_and_tcgetpgrp_are_consistent() {
        // Create a socket so the child can send us a byte if successful.
        let (mut rx, mut tx) = UnixStream::pair().unwrap();

        let child_pid = fork().unwrap();

        if child_pid == 0 {
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
            // If we haven't panicked yet, send a byte to the parent.
            tx.write_all(&[42]).unwrap();
        }

        drop(tx);

        // Read one byte from the children to comfirm that it did not panic.
        let mut buf = [0];
        rx.read_exact(&mut buf).unwrap();
        assert_eq!(buf[0], 42);
    }
}
