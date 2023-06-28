use std::{
    collections::{BTreeMap, HashMap},
    io,
    os::fd::{AsRawFd, RawFd},
};

use crate::cutils::cerr;
use libc::{c_short, pollfd, POLLIN, POLLOUT};

/// The kind of event that will be monitored for a file descriptor.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum PollEvent {
    /// Data may be read without blocking.
    Readable,
    /// Data may be written without blocking.
    Writable,
}

/// A set of indexed file descriptors to be polled using the [`poll`](https://manpage.me/?q=poll) system call.
pub struct PollSet<K> {
    fds: BTreeMap<K, (RawFd, bool, c_short)>,
}

impl<K: Eq + PartialEq + Ord + PartialOrd + Clone> PollSet<K> {
    /// Create an empty set of file descriptors.
    pub const fn new() -> Self {
        Self {
            fds: BTreeMap::new(),
        }
    }

    /// Add a file descriptor under the provided key. This descriptor will be checked for the given
    /// poll event and return a unique identifier for the descriptor inside the set.
    ///
    /// If the provided key is already in the set, calling this function will overwrite the file
    /// descriptor for that key.
    pub fn add_fd<F: AsRawFd>(&mut self, key: K, fd: &F, event: PollEvent) {
        let event = match event {
            PollEvent::Readable => POLLIN,
            PollEvent::Writable => POLLOUT,
        };
        self.fds.insert(key, (fd.as_raw_fd(), true, event));
    }

    /// Ignore the file descriptor under the provided key, if any.
    pub fn ignore_fd(&mut self, key: K) {
        if let Some((_, should_poll, _)) = self.fds.get_mut(&key) {
            *should_poll = false;
        }
    }

    /// Stop ignoring the file descriptor under the provided key, if any.
    pub fn resume_fd(&mut self, key: K) {
        if let Some((_, should_poll, _)) = self.fds.get_mut(&key) {
            *should_poll = true;
        }
    }

    /// Poll the file descriptors of the set that are not being ignored and return the key of the
    /// descriptors that are ready to be read or written.
    ///
    /// Calling this function will block until one of the file descriptors in the set is ready.
    pub fn poll(&mut self) -> io::Result<Vec<K>> {
        let (mut keys, mut fds): (Vec<K>, Vec<pollfd>) = self
            .fds
            .iter()
            .filter_map(|(key, &(fd, should_poll, events))| {
                should_poll.then(|| {
                    (
                        key.clone(),
                        pollfd {
                            fd,
                            events,
                            revents: 0,
                        },
                    )
                })
            })
            .unzip();

        // Don't call poll if there are no file descriptors to be polled.
        if keys.is_empty() {
            return Ok(keys);
        }

        // FIXME: we should set either a timeout or use ppoll when available.
        let n = cerr(unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, -1) })?;

        // Remove the keys that correspond to file descriptors that were not ready.
        for i in (0..keys.len()).rev() {
            let fd = fds[i];

            let events = fd.events & fd.revents;
            if !((events & POLLIN != 0) || (events & POLLOUT != 0)) {
                keys.remove(i);
            }
        }

        Ok(keys)
    }
}
