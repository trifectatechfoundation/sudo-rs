use std::{fs::File, io, os::fd::AsRawFd};

use crate::{
    cutils::cerr,
    system::interface::{GroupId, UserId},
};

mod sealed {
    use std::fs::File;

    pub(crate) trait Sealed {}

    impl Sealed for File {}
}

pub(crate) trait Chown: sealed::Sealed {
    fn chown(&self, uid: UserId, gid: GroupId) -> io::Result<()>;
}

impl Chown for File {
    fn chown(&self, owner: UserId, group: GroupId) -> io::Result<()> {
        let fd = self.as_raw_fd();

        cerr(unsafe { libc::fchown(fd, owner, group) }).map(|_| ())
    }
}
