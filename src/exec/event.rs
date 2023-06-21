use std::{io, os::fd::AsRawFd};

use crate::log::dev_error;
use crate::system::signal::SignalAction;
use crate::system::{
    poll::PollSet,
    signal::{SignalHandler, SignalInfo, SignalNumber},
};

use signal_hook::consts::*;

pub(super) trait EventClosure: Sized {
    /// Reason why the event loop should break.
    ///
    /// See [`EventDispatcher::set_break`] for more information.
    type Break;
    /// Reason why the event loop should exit.
    ///
    /// See [`EventDispatcher::set_exit`] for more information.
    type Exit;
    /// Operation that the closure must do when a signal arrives.
    fn on_signal(&mut self, info: SignalInfo, dispatcher: &mut EventDispatcher<Self>);
}

/// This macro ensures that we don't forget to set signal handlers.
macro_rules! define_signals {
    ($($signal:ident = $repr:literal,)*) => {
        /// The signals that we can handle.
        pub(super) const SIGNALS: &[SignalNumber] = &[$($signal,)*];

        impl<T: EventClosure> EventDispatcher<T> {
            /// Create a new and empty event handler.
            ///
            /// Calling this function also creates new signal handlers for the signals in
            /// [`SIGNALS`] and sets the callbacks for each one of them using the
            /// [`EventClosure::on_signal`] implementation.
            pub(super) fn new() -> io::Result<Self> {
                let mut dispatcher = Self {
                    signal_handlers: [$(SignalHandler::new($signal).map_err(|err| {
                        dev_error!(
                            "unable to set handler for {}",
                            super::signal_fmt($signal)
                        );
                        err
                    })?,)*],
                    poll_set: PollSet::new(),
                    callbacks: Vec::with_capacity(SIGNALS.len()),
                    status: Status::Continue,
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

enum Status<T: EventClosure> {
    Continue,
    Stop(StopReason<T>),
}

impl<T: EventClosure> Status<T> {
    fn is_break(&self) -> bool {
        matches!(self, Self::Stop(StopReason::Break(_)))
    }

    fn take_stop(&mut self) -> Option<StopReason<T>> {
        // If the status ends up to be `Continue`, we are replacing it by another `Continue`.
        let status = std::mem::replace(self, Self::Continue);
        match status {
            Status::Continue => None,
            Status::Stop(reason) => Some(reason),
        }
    }

    fn take_break(&mut self) -> Option<T::Break> {
        match self.take_stop()? {
            StopReason::Break(break_reason) => Some(break_reason),
            reason @ StopReason::Exit(_) => {
                // Replace back the status because it was not a `Break`.
                *self = Self::Stop(reason);
                None
            }
        }
    }

    fn take_exit(&mut self) -> Option<T::Exit> {
        match self.take_stop()? {
            reason @ StopReason::Break(_) => {
                // Replace back the status because it was not an `Exit`.
                *self = Self::Stop(reason);
                None
            }
            StopReason::Exit(exit_reason) => Some(exit_reason),
        }
    }
}

pub(super) enum StopReason<T: EventClosure> {
    Break(T::Break),
    Exit(T::Exit),
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct EventId(usize);

pub(super) type Callback<T> = fn(&mut T, &mut EventDispatcher<T>);

/// A type able to poll file descriptors and run callbacks when the descriptors are ready.
pub(super) struct EventDispatcher<T: EventClosure> {
    signal_handlers: [SignalHandler; SIGNALS.len()],
    poll_set: PollSet<EventId>,
    callbacks: Vec<Callback<T>>,
    status: Status<T>,
}

impl<T: EventClosure> EventDispatcher<T> {
    /// Set the `fd` descriptor to be polled for read events and set `callback` to be called if
    /// `fd` is ready.
    pub(super) fn set_read_callback<F: AsRawFd>(&mut self, fd: &F, callback: Callback<T>) {
        let id = EventId(self.callbacks.len());
        self.poll_set.add_fd_read(id, fd);
        self.callbacks.push(callback);
    }

    /// Set the `fd` descriptor to be polled for write events and set `callback` to be called if
    /// `fd` is ready.
    pub(super) fn set_write_callback<F: AsRawFd>(&mut self, fd: &F, callback: Callback<T>) {
        let id = EventId(self.callbacks.len());
        self.poll_set.add_fd_write(id, fd);
        self.callbacks.push(callback);
    }

    /// Stop the event loop when the current callback is done and set a reason for it.
    ///
    /// This means that the event loop will stop even if other events are ready.
    pub(super) fn set_break(&mut self, reason: T::Break) {
        self.status = Status::Stop(StopReason::Break(reason));
    }

    /// Stop the event loop when the callbacks for the events that are ready by now have been
    /// dispatched and set a reason for it.
    pub(super) fn set_exit(&mut self, reason: T::Exit) {
        self.status = Status::Stop(StopReason::Exit(reason));
    }

    /// Return whether a break reason has been set already. This function will return `false` after
    /// [`EventDispatcher::event_loop`] has been called.
    pub(super) fn got_break(&self) -> bool {
        self.status.is_break()
    }

    /// Run the event loop for this handler.
    ///
    /// The event loop will continue indefinitely unless you call [`EventDispatcher::set_break`] or
    /// [`EventDispatcher::set_exit`].
    pub(super) fn event_loop(&mut self, state: &mut T) -> StopReason<T> {
        loop {
            if let Ok(ids) = self.poll_set.poll() {
                for EventId(id) in ids {
                    self.callbacks[id](state, self);

                    if let Some(reason) = self.status.take_break() {
                        return StopReason::Break(reason);
                    }
                }
                if let Some(reason) = self.status.take_exit() {
                    return StopReason::Exit(reason);
                }
            } else {
                // FIXME: maybe we shout return the IO error instead.
                if let Some(reason) = self.status.take_stop() {
                    return reason;
                }
            }
        }
    }

    /// Unregister all the handlers created by the dispatcher.
    pub(super) fn unregister_handlers(self) {
        for handler in self.signal_handlers {
            handler.unregister();
        }
    }

    /// Set the signal action for a specific signal handler.
    pub(super) fn set_signal_action(&mut self, signal: SignalNumber, action: SignalAction) {
        if let Some(i) = SIGNALS
            .iter()
            .enumerate()
            .find_map(|(i, &sig)| (signal == sig).then_some(i))
        {
            self.signal_handlers[i].set_action(action);
        }
    }
}
