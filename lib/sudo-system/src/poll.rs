use std::{io, os::fd::AsRawFd};

use libc::{c_short, pollfd, POLLIN, POLLOUT};
use sudo_cutils::cerr;

#[derive(Eq, PartialEq, Hash)]
pub struct PollIdx(usize);

pub struct PollSet {
    fds: Vec<pollfd>,
}

impl PollSet {
    pub const fn new() -> Self {
        Self { fds: vec![] }
    }

    pub fn add_fd_read<F: AsRawFd>(&mut self, fd: &F) -> PollIdx {
        self.add_fd(fd, POLLIN)
    }

    pub fn add_fd_write<F: AsRawFd>(&mut self, fd: &F) -> PollIdx {
        self.add_fd(fd, POLLOUT)
    }

    fn add_fd<F: AsRawFd>(&mut self, fd: &F, events: c_short) -> PollIdx {
        let idx = PollIdx(self.fds.len());
        self.fds.push(pollfd {
            fd: fd.as_raw_fd(),
            events,
            revents: 0,
        });
        idx
    }

    pub fn poll(&mut self) -> io::Result<Vec<PollIdx>> {
        let n = cerr(unsafe { libc::poll(self.fds.as_mut_ptr(), self.fds.len() as _, -1) })?;
        let mut idxs = Vec::with_capacity(n as usize);

        for (idx, fd) in self.fds.iter_mut().enumerate() {
            let events = fd.events & fd.revents;

            if (events & POLLIN != 0) || (events & POLLOUT != 0) {
                idxs.push(PollIdx(idx));
            }

            fd.revents = 0;
        }

        Ok(idxs)
    }
}
