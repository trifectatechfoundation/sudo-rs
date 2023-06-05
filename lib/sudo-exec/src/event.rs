use std::os::fd::AsRawFd;

use sudo_system::poll::PollSet;

#[derive(PartialEq, Eq, Hash, Clone)]
struct EventId(usize);

pub(crate) type Callback<T> = fn(&mut T, &mut EventHandler<T>);

/// A type able to poll events from file descriptors and run callbacks when events are ready.
pub(crate) struct EventHandler<T> {
    poll_set: PollSet<EventId>,
    callbacks: Vec<Callback<T>>,
    brk: bool,
}

impl<T> EventHandler<T> {
    /// Create a new and empty event handler.
    pub(crate) fn new() -> Self {
        Self {
            poll_set: PollSet::new(),
            callbacks: Vec::new(),
            brk: false,
        }
    }

    /// Set the `fd` descriptor to be polled for read events and set `callback` to be called if
    /// `fd` is ready.  
    pub(crate) fn set_read_callback<F: AsRawFd>(&mut self, fd: &F, callback: Callback<T>) {
        let id = EventId(self.callbacks.len());
        self.poll_set.add_fd_read(id, fd);
        self.callbacks.push(callback);
    }

    /// Stop the event loop when the current callback is done.
    ///
    /// This means that the event loop will stop even if other events are ready.
    pub(crate) fn set_break(&mut self) {
        self.brk = true;
    }

    /// Run the event loop for this handler.
    ///
    /// The event loop will continue indefinitely unless either [`EventHandler::set_exit`]  or
    /// [`EventHandler::set_break`] are called.
    pub(crate) fn event_loop(&mut self, state: &mut T) {
        loop {
            if let Ok(ids) = self.poll_set.poll() {
                for EventId(id) in ids {
                    self.callbacks[id](state, self);

                    if self.brk {
                        return;
                    }
                }
            }
            if self.brk {
                return;
            }
        }
    }
}
