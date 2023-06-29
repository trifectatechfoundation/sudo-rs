use std::{fs::File, io::Result, os::fd::AsRawFd};

use crate::cutils::cerr;

pub trait Lockable {
    /// Get an exclusive lock on the file, waits if there is currently a lock
    /// on the file
    fn lock_exclusive(&self) -> Result<()>;

    /// Get a shared lock on the file, waits if there is currently an exclusive
    /// lock on the file.
    fn lock_shared(&self) -> Result<()>;

    /// Release the lock on the file if there is any.
    fn unlock(&self) -> Result<()>;
}

#[derive(Clone, Copy, Debug)]
enum LockOp {
    LockExclusive,
    LockShared,
    Unlock,
}

impl LockOp {
    fn as_flock_operation(self) -> libc::c_int {
        match self {
            LockOp::LockExclusive => libc::LOCK_EX,
            LockOp::LockShared => libc::LOCK_SH,
            LockOp::Unlock => libc::LOCK_UN,
        }
    }
}

fn flock(fd: &impl AsRawFd, action: LockOp, blocking: bool) -> Result<()> {
    let mut operation = action.as_flock_operation();
    if !blocking {
        operation |= libc::LOCK_NB;
    }

    cerr(unsafe { libc::flock(fd.as_raw_fd(), operation) })?;
    Ok(())
}

impl Lockable for File {
    fn lock_exclusive(&self) -> Result<()> {
        flock(self, LockOp::LockExclusive, true)
    }

    fn lock_shared(&self) -> Result<()> {
        flock(self, LockOp::LockShared, true)
    }

    fn unlock(&self) -> Result<()> {
        flock(self, LockOp::Unlock, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Lockable for std::io::Cursor<Vec<u8>> {
        fn lock_exclusive(&self) -> Result<()> {
            Ok(())
        }

        fn lock_shared(&self) -> Result<()> {
            Ok(())
        }

        fn unlock(&self) -> Result<()> {
            Ok(())
        }
    }

    impl Lockable for std::io::Cursor<&mut Vec<u8>> {
        fn lock_exclusive(&self) -> Result<()> {
            Ok(())
        }

        fn lock_shared(&self) -> Result<()> {
            Ok(())
        }

        fn unlock(&self) -> Result<()> {
            Ok(())
        }
    }

    fn tempfile() -> std::io::Result<std::fs::File> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Failed to get system time")
            .as_nanos();
        let pid = std::process::id();

        let filename = format!("sudo_rs_test_{}_{}", pid, timestamp);
        let path = std::path::PathBuf::from("/tmp").join(filename);
        std::fs::File::create(path)
    }

    #[test]
    fn test_locking_of_tmp_file() {
        let f = tempfile().unwrap();
        assert!(f.lock_shared().is_ok());
        assert!(f.unlock().is_ok());
        assert!(f.lock_exclusive().is_ok());
        assert!(f.unlock().is_ok());
    }
}
