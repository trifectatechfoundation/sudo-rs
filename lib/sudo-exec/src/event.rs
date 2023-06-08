use std::{collections::HashMap, ffi::c_int, io, os::fd::AsRawFd};

use sudo_system::{
    poll::PollSet,
    signal::{SignalHandler, SignalInfo},
};

use signal_hook::consts::*;

pub(crate) trait RelaySignal: Sized {
    fn relay_signal(&mut self, info: SignalInfo, ev: &mut EventHandler<Self>);
}

/// This macro ensures that we don't forget to set signal handlers.
macro_rules! define_signals {
    ($($signal:ident,)*) => {
        impl<T: RelaySignal> EventHandler<T> {
            /// The signals that we can handle.
            pub(crate) const SIGNALS: &[c_int] = &[$($signal,)*];

            /// Create a new and empty event handler.
            ///
            /// Calling this function also creates new signal handlers for the signals in
            /// [`SIGNALS`] and sets the callbacks for each one of them using the `RelaySignal`
            /// implementation.
            pub(crate) fn new() -> io::Result<Self> {
                let mut ev = Self {
                    signal_handlers: HashMap::with_capacity(Self::SIGNALS.len()),
                    poll_set: PollSet::new(),
                    callbacks: Vec::new(),
                    brk: false,
                };

                $(
                    let handler = SignalHandler::new($signal)?;
                    ev.set_read_callback(&handler, |t, ev| {
                        let handler = ev.signal_handlers.get_mut(&$signal).unwrap();
                        if let Ok(info) = handler.recv() {
                            t.relay_signal(info, ev);
                        }
                    });
                    ev.signal_handlers.insert($signal, handler);
                )*

                Ok(ev)
            }
        }
    };

}

define_signals! {
    SIGINT, SIGQUIT, SIGTSTP, SIGTERM, SIGHUP, SIGALRM, SIGPIPE, SIGUSR1, SIGUSR2, SIGCHLD,
    SIGCONT, SIGWINCH,
}

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
