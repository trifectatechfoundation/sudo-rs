mod user_term;

use std::{
    ffi::{c_uchar, CStr, CString, OsStr, OsString},
    fs::File,
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::prelude::OsStrExt,
    },
    ptr::null_mut,
};

use crate::cutils::{cerr, os_string_from_ptr};

use super::interface::ProcessId;

pub(crate) use user_term::UserTerm;

pub(crate) struct Pty {
    /// The file path of the leader side of the pty.
    pub(crate) path: CString,
    /// The leader side of the pty.
    pub(crate) leader: PtyLeader,
    /// The follower side of the pty.
    pub(crate) follower: PtyFollower,
}

impl Pty {
    pub(crate) fn open() -> io::Result<Self> {
        const PATH_MAX: usize = libc::PATH_MAX as _;
        // Allocate a buffer to hold the path to the pty.
        let mut path = vec![0 as c_uchar; PATH_MAX];
        // Create two integers to hold the file descriptors for each side of the pty.
        let (mut leader, mut follower) = (0, 0);

        cerr(unsafe {
            libc::openpty(
                &mut leader,
                &mut follower,
                path.as_mut_ptr().cast(),
                null_mut::<libc::termios>(),
                null_mut::<libc::winsize>(),
            )
        })?;

        // Get the index of the first null byte and truncate `path` so it doesn't have any null
        // bytes. If there are no null bytes the path is left as it is.
        if let Some(index) = path
            .iter()
            .enumerate()
            .find_map(|(index, &byte)| (byte == 0).then_some(index))
        {
            path.truncate(index);
        }

        // This will not panic because `path` was truncated to not have any null bytes.
        let path = CString::new(path).unwrap();

        Ok(Self {
            path,
            leader: PtyLeader {
                file: unsafe { OwnedFd::from_raw_fd(leader) }.into(),
            },
            follower: PtyFollower {
                file: unsafe { OwnedFd::from_raw_fd(follower) }.into(),
            },
        })
    }
}

pub(crate) struct PtyLeader {
    file: File,
}

impl io::Read for PtyLeader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

impl io::Write for PtyLeader {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

impl AsRawFd for PtyLeader {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.file.as_raw_fd()
    }
}

pub(crate) struct PtyFollower {
    file: File,
}

impl PtyFollower {
    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        self.file.try_clone().map(|file| Self { file })
    }
}

impl AsRawFd for PtyFollower {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.file.as_raw_fd()
    }
}

impl From<PtyFollower> for std::process::Stdio {
    fn from(follower: PtyFollower) -> Self {
        follower.file.into()
    }
}

mod sealed {
    use std::os::fd::AsRawFd;

    pub(crate) trait Sealed {}

    impl<F: AsRawFd> Sealed for F {}
}

pub(crate) trait Terminal: sealed::Sealed {
    fn tcgetpgrp(&self) -> io::Result<ProcessId>;
    fn tcsetpgrp(&self, pgrp: ProcessId) -> io::Result<()>;
    fn make_controlling_terminal(&self) -> io::Result<()>;
    fn ttyname(&self) -> io::Result<OsString>;
}

impl<F: AsRawFd> Terminal for F {
    /// Get the foreground process group ID associated with this terminal.
    fn tcgetpgrp(&self) -> io::Result<ProcessId> {
        cerr(unsafe { libc::tcgetpgrp(self.as_raw_fd()) })
    }
    /// Set the foreground process group ID associated with this terminalto `pgrp`.
    fn tcsetpgrp(&self, pgrp: ProcessId) -> io::Result<()> {
        cerr(unsafe { libc::tcsetpgrp(self.as_raw_fd(), pgrp) }).map(|_| ())
    }

    /// Make the given terminal the controlling terminal of the calling process.
    fn make_controlling_terminal(&self) -> io::Result<()> {
        cerr(unsafe { libc::ioctl(self.as_raw_fd(), libc::TIOCSCTTY, 0) })?;
        Ok(())
    }

    /// Get the filename of the tty
    fn ttyname(&self) -> io::Result<OsString> {
        use std::os::unix::ffi::OsStringExt;

        let mut buf: [libc::c_char; 1024] = [0; 1024];

        cerr(unsafe { libc::ttyname_r(self.as_raw_fd(), buf.as_mut_ptr() as _, buf.len()) })?;
        Ok(unsafe { os_string_from_ptr(buf.as_ptr()) })
    }
}

/// Try to get the path of the current TTY
pub fn current_tty_name() -> io::Result<OsString> {
    std::io::stdin().ttyname()
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        io::{IsTerminal, Read, Write},
        os::unix::{net::UnixStream, prelude::OsStringExt},
        path::PathBuf,
        process::exit,
    };

    use crate::system::{fork, getpgid, setsid, term::*, ForkResult};

    #[test]
    fn open_pty() {
        let pty = Pty::open().unwrap();
        assert!(pty.leader.file.is_terminal());
        assert!(pty.follower.file.is_terminal());

        let path = PathBuf::from(OsString::from_vec(pty.path.into_bytes()));
        assert!(path.try_exists().unwrap());
        assert!(path.starts_with("/dev/pts/"));
    }

    #[test]
    fn tcsetpgrp_and_tcgetpgrp_are_consistent() {
        // Create a socket so the child can send us a byte if successful.
        let (mut rx, mut tx) = UnixStream::pair().unwrap();

        let ForkResult::Parent(child_pid) = fork().unwrap() else {
            // Open a new pseudoterminal.
            let leader = Pty::open().unwrap().leader;
            // The pty leader should not have a foreground process group yet.
            assert_eq!(leader.tcgetpgrp().unwrap(), 0);
            // Create a new session so we can change the controlling terminal.
            setsid().unwrap();
            // Set the pty leader as the controlling terminal.
            leader.make_controlling_terminal().unwrap();
            // Set us as the foreground process group of the pty leader.
            let pgid = getpgid(0).unwrap();
            leader.tcsetpgrp(pgid).unwrap();
            // Check that we are in fact the foreground process group of the pty leader.
            assert_eq!(pgid, leader.tcgetpgrp().unwrap());
            // If we haven't panicked yet, send a byte to the parent.
            tx.write_all(&[42]).unwrap();

            exit(0);
        };

        drop(tx);

        // Read one byte from the children to comfirm that it did not panic.
        let mut buf = [0];
        rx.read_exact(&mut buf).unwrap();
        assert_eq!(buf[0], 42);
    }
}
