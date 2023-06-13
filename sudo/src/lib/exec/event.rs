use std::{io, ops::ControlFlow, os::fd::AsRawFd};

use sudo_system::{
    poll::PollSet,
    signal::{SignalHandler, SignalInfo, SignalNumber},
};

use signal_hook::consts::*;

pub(crate) trait EventClosure: Sized {
    /// Reason why the event loop should break. This is the return type of [`EventDispatcher::event_loop`].
    type Break;
    /// Operation that the closure must do when a signal arrives.
    fn on_signal(&mut self, info: SignalInfo, dispatcher: &mut EventDispatcher<Self>);
}

/// This macro ensures that we don't forget to set signal handlers.
macro_rules! define_signals {
    ($($signal:ident = $repr:literal,)*) => {
        /// The signals that we can handle.
        pub(crate) const SIGNALS: &[SignalNumber] = &[$($signal,)*];

        impl<T: EventClosure> EventDispatcher<T> {
            /// Create a new and empty event handler.
            ///
            /// Calling this function also creates new signal handlers for the signals in
            /// [`SIGNALS`] and sets the callbacks for each one of them using the
            /// [`EventClosure::on_signal`] implementation.
            pub(crate) fn new() -> io::Result<Self> {
                let mut dispatcher = Self {
                    signal_handlers: [$(SignalHandler::new($signal)?,)*],
                    poll_set: PollSet::new(),
                    callbacks: Vec::with_capacity(SIGNALS.len()),
                    status: ControlFlow::Continue(()),
                };

                $(
                    let handler = &dispatcher.signal_handlers[$repr].as_raw_fd();
                    dispatcher.set_read_callback(handler, |closure, dispatcher| {
                        let handler = &mut dispatcher.signal_handlers[$repr];
                        if let Ok(info) = handler.recv() {
                            closure.on_signal(info, dispatcher);
                        }
                    });
                )*

                Ok(dispatcher)
            }
        }
    };

}

define_signals! {
    SIGINT = 0,
    SIGQUIT = 1,
    SIGTSTP = 2,
    SIGTERM = 3,
    SIGHUP = 4,
    SIGALRM = 5,
    SIGPIPE = 6,
    SIGUSR1 = 7,
    SIGUSR2 = 8,
    SIGCHLD = 9,
    SIGCONT = 10,
    SIGWINCH = 11,
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct EventId(usize);

pub(crate) type Callback<T> = fn(&mut T, &mut EventDispatcher<T>);

/// A type able to poll file descriptors and run callbacks when the descriptors are ready.
pub(crate) struct EventDispatcher<T: EventClosure> {
    signal_handlers: [SignalHandler; SIGNALS.len()],
    poll_set: PollSet<EventId>,
    callbacks: Vec<Callback<T>>,
    status: ControlFlow<T::Break>,
}

impl<T: EventClosure> EventDispatcher<T> {
    /// Set the `fd` descriptor to be polled for read events and set `callback` to be called if
    /// `fd` is ready.  
    pub(crate) fn set_read_callback<F: AsRawFd>(&mut self, fd: &F, callback: Callback<T>) {
        let id = EventId(self.callbacks.len());
        self.poll_set.add_fd_read(id, fd);
        self.callbacks.push(callback);
    }

    /// Set the `fd` descriptor to be polled for write events and set `callback` to be called if
    /// `fd` is ready.  
    pub(crate) fn set_write_callback<F: AsRawFd>(&mut self, fd: &F, callback: Callback<T>) {
        let id = EventId(self.callbacks.len());
        self.poll_set.add_fd_write(id, fd);
        self.callbacks.push(callback);
    }

    /// Stop the event loop when the current callback is done and set a reason for it.
    ///
    /// This means that the event loop will stop even if other events are ready.
    pub(crate) fn set_break(&mut self, reason: T::Break) {
        self.status = ControlFlow::Break(reason);
    }

    /// Return whether a break reason has been set already. This function will return `false` after
    /// [`EventDispatcher::event_loop`] has been called.
    pub(crate) fn got_break(&self) -> bool {
        self.status.is_break()
    }

    /// Run the event loop for this handler.
    ///
    /// The event loop will continue indefinitely unless either [`EventDispatcher::set_exit`]  or
    /// [`EventDispatcher::set_break`] are called.
    pub(crate) fn event_loop(&mut self, state: &mut T) -> T::Break {
        loop {
            if let Ok(ids) = self.poll_set.poll() {
                for EventId(id) in ids {
                    self.callbacks[id](state, self);

                    if let Some(break_reason) = self.check_break() {
                        return break_reason;
                    }
                }
            }

            if let Some(break_reason) = self.check_break() {
                return break_reason;
            }
        }
    }

    pub(crate) fn check_break(&mut self) -> Option<T::Break> {
        // This is OK as we are swapping `Continue(())` by other `Continue(())` if the status is
        // not `Break`.
        match std::mem::replace(&mut self.status, ControlFlow::Continue(())) {
            ControlFlow::Continue(()) => None,
            ControlFlow::Break(reason) => Some(reason),
        }
    }
}
