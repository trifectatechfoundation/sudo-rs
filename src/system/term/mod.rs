mod user_term;

use std::{
    ffi::{c_uchar, CString, OsString},
    fmt,
    fs::File,
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    ptr::null_mut,
};

use libc::{ioctl, winsize, TIOCSWINSZ};

use crate::cutils::{cerr, os_string_from_ptr, safe_isatty};

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

        // SAFETY:
        // - openpty is passed two valid pointers as its first two arguments
        // - path is a valid array that can hold PATH_MAX characters; and casting `u8` to `i8` is
        //   valid since all values are initialized to zero.
        // - the last two arguments are allowed to be NULL
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
                // SAFETY: `openpty` has set `leader` to an open fd suitable for assuming ownership by `OwnedFd`.
                file: unsafe { OwnedFd::from_raw_fd(leader) }.into(),
            },
            follower: PtyFollower {
                // SAFETY: `openpty` has set `follower` to an open fd suitable for assuming ownership by `OwnedFd`.
                file: unsafe { OwnedFd::from_raw_fd(follower) }.into(),
            },
        })
    }
}

pub(crate) struct PtyLeader {
    file: File,
}

impl PtyLeader {
    pub(crate) fn set_size(&self, term_size: &TermSize) -> io::Result<()> {
        // SAFETY: the TIOCSWINSZ expects an initialized pointer of type `winsize`
        // https://www.man7.org/linux/man-pages/man2/TIOCSWINSZ.2const.html
        //
        // An object of type TermSize is safe to cast to `winsize` since it is a
        // repr(transparent) "newtype" struct.
        cerr(unsafe {
            ioctl(
                self.file.as_raw_fd(),
                TIOCSWINSZ,
                (term_size as *const TermSize).cast::<libc::winsize>(),
            )
        })?;

        Ok(())
    }
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
    fn is_terminal(&self) -> bool;
    fn tcgetsid(&self) -> io::Result<ProcessId>;
}

impl<F: AsRawFd> Terminal for F {
    /// Get the foreground process group ID associated with this terminal.
    fn tcgetpgrp(&self) -> io::Result<ProcessId> {
        // SAFETY: tcgetpgrp cannot cause UB
        let id = cerr(unsafe { libc::tcgetpgrp(self.as_raw_fd()) })?;
        Ok(ProcessId::new(id))
    }
    /// Set the foreground process group ID associated with this terminal to `pgrp`.
    fn tcsetpgrp(&self, pgrp: ProcessId) -> io::Result<()> {
        // SAFETY: tcsetpgrp cannot cause UB
        cerr(unsafe { libc::tcsetpgrp(self.as_raw_fd(), pgrp.inner()) }).map(|_| ())
    }

    /// Make the given terminal the controlling terminal of the calling process.
    fn make_controlling_terminal(&self) -> io::Result<()> {
        // SAFETY: this is a correct way to call the TIOCSCTTY ioctl, see:
        // https://www.man7.org/linux/man-pages/man2/TIOCNOTTY.2const.html
        cerr(unsafe { libc::ioctl(self.as_raw_fd(), libc::TIOCSCTTY as _, 0) })?;
        Ok(())
    }

    /// Get the filename of the tty
    fn ttyname(&self) -> io::Result<OsString> {
        let mut buf: [libc::c_char; 1024] = [0; 1024];

        if !safe_isatty(self.as_raw_fd()) {
            return Err(io::ErrorKind::Unsupported.into());
        }

        // SAFETY: `buf` is a valid and initialized pointer, and its  correct length is passed
        cerr(unsafe { libc::ttyname_r(self.as_raw_fd(), buf.as_mut_ptr(), buf.len()) })?;
        // SAFETY: `buf` will have been initialized by the `ttyname_r` call, if it succeeded
        Ok(unsafe { os_string_from_ptr(buf.as_ptr()) })
    }

    /// Rust standard library "IsTerminal" is not secure for setuid programs (CVE-2023-2002)
    fn is_terminal(&self) -> bool {
        safe_isatty(self.as_raw_fd())
    }

    fn tcgetsid(&self) -> io::Result<ProcessId> {
        // SAFETY: tcgetsid cannot cause UB
        let id = cerr(unsafe { libc::tcgetsid(self.as_raw_fd()) })?;
        Ok(ProcessId::new(id))
    }
}

/// Try to get the path of the current TTY
pub fn current_tty_name() -> io::Result<OsString> {
    std::io::stdin().ttyname()
}

#[repr(transparent)]
pub(crate) struct TermSize {
    raw: winsize,
}

impl PartialEq for TermSize {
    fn eq(&self, other: &Self) -> bool {
        self.raw.ws_col == other.raw.ws_col && self.raw.ws_row == other.raw.ws_row
    }
}

impl fmt::Display for TermSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} x {}", self.raw.ws_row, self.raw.ws_col)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        io::{Read, Write},
        os::unix::{net::UnixStream, prelude::OsStringExt},
        path::PathBuf,
        process::exit,
    };

    use crate::system::{fork_for_test, getpgid, setsid, term::*};

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

        unsafe {
            fork_for_test(|| {
                // Open a new pseudoterminal.
                let leader = Pty::open().unwrap().leader;
                // On FreeBSD this returns an unspecified PID when there is no foreground process
                // group, so skip this check on FreeBSD.
                if cfg!(not(target_os = "freebsd")) {
                    // The pty leader should not have a foreground process group yet.
                    assert_eq!(leader.tcgetpgrp().unwrap().inner(), 0);
                }
                // Create a new session so we can change the controlling terminal.
                setsid().unwrap();
                // Set the pty leader as the controlling terminal.
                leader.make_controlling_terminal().unwrap();
                // Set us as the foreground process group of the pty leader.
                let pgid = getpgid(ProcessId::new(0)).unwrap();
                leader.tcsetpgrp(pgid).unwrap();
                // Check that we are in fact the foreground process group of the pty leader.
                assert_eq!(pgid, leader.tcgetpgrp().unwrap());
                // If we haven't panicked yet, send a byte to the parent.
                tx.write_all(&[42]).unwrap();

                exit(0);
            })
        };

        drop(tx);

        // Read one byte from the children to comfirm that it did not panic.
        let mut buf = [0];
        rx.read_exact(&mut buf).unwrap();
        assert_eq!(buf[0], 42);
    }
}
