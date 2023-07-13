use std::{
    fs::File,
    io::Result,
    os::fd::{AsRawFd, RawFd},
};

use crate::cutils::cerr;

pub(crate) struct FileLock {
    fd: RawFd,
}

impl FileLock {
    /// Get an exclusive lock on the file, waits if there is currently a lock
    /// on the file if `nonblocking` is true.
    pub(crate) fn exclusive(file: &File, nonblocking: bool) -> Result<Self> {
        let fd = file.as_raw_fd();
        flock(fd, LockOp::LockExclusive, nonblocking)?;
        Ok(Self { fd })
    }

    /// Release the lock on the file.
    pub(crate) fn unlock(self) -> Result<()> {
        flock(self.fd, LockOp::Unlock, false)
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        flock(self.fd, LockOp::Unlock, false).ok();
    }
}

#[derive(Clone, Copy, Debug)]
enum LockOp {
    LockExclusive,
    Unlock,
}

impl LockOp {
    fn as_flock_operation(self) -> libc::c_int {
        match self {
            LockOp::LockExclusive => libc::LOCK_EX,
            LockOp::Unlock => libc::LOCK_UN,
        }
    }
}

fn flock(fd: RawFd, action: LockOp, nonblocking: bool) -> Result<()> {
    let mut operation = action.as_flock_operation();
    if nonblocking {
        operation |= libc::LOCK_NB;
    }

    cerr(unsafe { libc::flock(fd, operation) })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::system::tests::tempfile;

    use super::*;

    #[test]
    fn test_locking_of_tmp_file() {
        let f = tempfile().unwrap();

        FileLock::exclusive(&f, false).unwrap().unlock().unwrap();
    }
}
