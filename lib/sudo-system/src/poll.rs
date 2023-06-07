use std::{
    collections::HashMap,
    hash::Hash,
    io,
    os::fd::{AsRawFd, RawFd},
};

use libc::{c_short, pollfd, POLLIN, POLLOUT};
use sudo_cutils::cerr;

/// A set of indexed file descriptors to be polled using the [`poll`](https://manpage.me/?q=poll) system call.
pub struct PollSet<K> {
    fds: HashMap<K, (RawFd, c_short)>,
}

impl<K: Eq + PartialEq + Hash + Clone> Default for PollSet<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + PartialEq + Hash + Clone> PollSet<K> {
    /// Create an empty set of file descriptors.
    pub fn new() -> Self {
        Self {
            fds: HashMap::new(),
        }
    }

    /// Add a file descriptor under the provided key. This descriptor will be checked for read events and return a unique identifier
    /// for the descriptor inside the set.
    ///
    /// If the provided key is already in the set, calling this function will overwrite the file
    /// descriptor for that key.
    pub fn add_fd_read<F: AsRawFd>(&mut self, key: K, fd: &F) {
        self.add_fd(key, fd, POLLIN)
    }

    /// Add a file descriptor under the provided key. This descriptor will be checked for write events and return a unique identifier
    /// for the descriptor inside the set.
    ///
    /// If the provided key is already in the set, calling this function will overwrite the file
    /// descriptor for that key.
    pub fn add_fd_write<F: AsRawFd>(&mut self, key: K, fd: &F) {
        self.add_fd(key, fd, POLLOUT)
    }

    fn add_fd<F: AsRawFd>(&mut self, key: K, fd: &F, events: c_short) {
        self.fds.insert(key, (fd.as_raw_fd(), events));
    }

    /// Poll the set of file descriptors and return the key of the descriptors that are ready to be
    /// read or written.
    ///
    /// Calling this function will block until one of the file descriptors in the set is ready.
    pub fn poll(&mut self) -> io::Result<Vec<K>> {
        let mut fds: Vec<pollfd> = self
            .fds
            .values()
            .map(|&(fd, events)| pollfd {
                fd,
                events,
                revents: 0,
            })
            .collect();

        // FIXME: we should set either a timeout or use ppoll when available.
        let n = cerr(unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, -1) })?;

        let mut keys = Vec::with_capacity(n as usize);

        for (key, fd) in self.fds.keys().zip(fds) {
            let events = fd.events & fd.revents;

            if (events & POLLIN != 0) || (events & POLLOUT != 0) {
                keys.push(key.clone());
            }
        }

        Ok(keys)
    }
}
