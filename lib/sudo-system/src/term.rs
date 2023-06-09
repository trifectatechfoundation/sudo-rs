use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    ptr::null_mut,
};

use sudo_cutils::cerr;

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
