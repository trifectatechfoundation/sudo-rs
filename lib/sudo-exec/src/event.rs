use std::{collections::HashMap, ffi::c_int, io, os::fd::AsRawFd};

use sudo_system::{
    poll::PollSet,
    signal::{SignalHandler, SignalInfo},
};

use signal_hook::consts::*;

pub(crate) const SIGNALS: &[c_int] = &[
    SIGINT, SIGQUIT, SIGTSTP, SIGTERM, SIGHUP, SIGALRM, SIGPIPE, SIGUSR1, SIGUSR2, SIGCHLD,
    SIGCONT, SIGWINCH,
];

#[derive(PartialEq, Eq, Hash, Clone)]
struct EventId(usize);

pub(crate) type Callback<T> = fn(&mut T, &mut EventHandler<T>);

/// A type able to poll events from file descriptors and run callbacks when events are ready.
pub(crate) struct EventHandler<T> {
    signal_handlers: HashMap<c_int, SignalHandler>,
    poll_set: PollSet<EventId>,
    callbacks: Vec<Callback<T>>,
    brk: bool,
}

impl<T> EventHandler<T> {
    /// Create a new and empty event handler.
    ///
    /// Calling this function also creates new signal handlers for the signals in [`SIGNALS`].
    pub(crate) fn new() -> io::Result<Self> {
        let mut signal_handlers = HashMap::with_capacity(SIGNALS.len());
        for &signal in SIGNALS {
            signal_handlers.insert(signal, SignalHandler::new(signal)?);
        }

        Ok(Self {
            signal_handlers,
            poll_set: PollSet::new(),
            callbacks: Vec::new(),
            brk: false,
        })
    }

    /// Set the `fd` descriptor to be polled for read events and set `callback` to be called if
    /// `fd` is ready.  
    pub(crate) fn set_read_callback<F: AsRawFd>(&mut self, fd: &F, callback: Callback<T>) {
        let id = EventId(self.callbacks.len());
        self.poll_set.add_fd_read(id, fd);
        self.callbacks.push(callback);
    }

    /// Set the handler for `SIGNAL` to be polled for read events and set `callback` to be called
    /// if the handler is ready.  
    pub(crate) fn set_signal_callback<const SIGNAL: c_int>(&mut self, callback: Callback<T>) {
        let id = EventId(self.callbacks.len());
        let Some(handler) = self.signal_handlers.get(&SIGNAL) else { 
            return;
        };
        self.poll_set.add_fd_read(id, handler);
        self.callbacks.push(callback);
    }

    /// Receive the information related to the arrival of a signal with number `SIGNAL`. Return
    /// `None` if the signal is not in [`SIGNALS`].
    ///
    /// Calling this function will block until a signal with number `SIGNAL` arrives.
    pub(crate) fn recv_signal_info<const SIGNAL: c_int>(
        &mut self,
    ) -> Option<io::Result<SignalInfo>> {
        self.signal_handlers.get_mut(&SIGNAL).map(|h| h.recv())
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
