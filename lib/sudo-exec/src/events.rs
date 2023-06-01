use std::{collections::HashMap, os::fd::AsRawFd};

use sudo_system::poll::{PollIdx, PollSet};

pub(crate) struct EventQueue<C> {
    poll_set: PollSet,
    callbacks: HashMap<PollIdx, fn(&mut C, &mut Self)>,
    exit: bool,
    brk: bool,
}

impl<C> EventQueue<C> {
    pub(crate) fn new() -> Self {
        Self {
            poll_set: PollSet::new(),
            callbacks: HashMap::new(),
            exit: false,
            brk: false,
        }
    }

    pub(crate) fn add_write_event<F: AsRawFd>(&mut self, fd: &F, cb: fn(&mut C, &mut Self)) {
        let idx = self.poll_set.add_fd_write(fd);
        self.callbacks.insert(idx, cb);
    }

    pub(crate) fn add_read_event<F: AsRawFd>(&mut self, fd: &F, cb: fn(&mut C, &mut Self)) {
        let idx = self.poll_set.add_fd_read(fd);
        self.callbacks.insert(idx, cb);
    }

    pub(crate) fn set_exit(&mut self) {
        if !self.brk {
            self.exit = true;
        }
    }

    pub(crate) fn set_break(&mut self) {
        self.exit = false;
        self.brk = true;
    }

    pub(crate) fn set_continue(&mut self) {
        if !self.brk {
            self.exit = false;
        }
    }

    pub(crate) fn start_loop(&mut self, closure: &mut C) {
        loop {
            if let Ok(idxs) = self.poll_set.poll() {
                for idx in idxs {
                    self.callbacks[&idx](closure, self);

                    if self.brk {
                        return;
                    }
                }

                if self.exit {
                    return;
                }
            }
        }
    }

    pub(crate) fn got_break(&self) -> bool {
        self.brk
    }
}
