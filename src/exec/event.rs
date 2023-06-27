use std::os::fd::AsRawFd;

use crate::system::poll::PollSet;

pub(super) trait Process: Sized {
    /// IO Events that this process should handle.
    type Event: Copy;
    /// Reason why the event loop should break.
    ///
    /// See [`EventDispatcher::set_break`] for more information.
    type Break;
    /// Reason why the event loop should exit.
    ///
    /// See [`EventDispatcher::set_exit`] for more information.
    type Exit;
    /// Handle the corresponding event.
    fn on_event(&mut self, event: Self::Event, dispatcher: &mut EventDispatcher<Self>);
}

enum Status<T: Process> {
    Continue,
    Stop(StopReason<T>),
}

impl<T: Process> Status<T> {
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

pub(super) enum StopReason<T: Process> {
    Break(T::Break),
    Exit(T::Exit),
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct EventId(usize);

/// A type able to poll file descriptors and run callbacks when the descriptors are ready.
pub(super) struct EventDispatcher<T: Process> {
    poll_set: PollSet<EventId>,
    events: Vec<T::Event>,
    status: Status<T>,
}

impl<T: Process> EventDispatcher<T> {
    /// Create a new and empty event handler.
    pub(super) fn new() -> Self {
        Self {
            poll_set: PollSet::new(),
            events: Vec::new(),
            status: Status::Continue,
        }
    }

    /// Set the `fd` descriptor to be polled for read events and set `callback` to be called if
    /// `fd` is ready.
    pub(super) fn register_read_event<F: AsRawFd>(&mut self, fd: &F, event: T::Event) {
        let id = EventId(self.events.len());
        self.poll_set.add_fd_read(id, fd);
        self.events.push(event);
    }

    /// Set the `fd` descriptor to be polled for write events and set `callback` to be called if
    /// `fd` is ready.
    pub(super) fn register_write_event<F: AsRawFd>(&mut self, fd: &F, event: T::Event) {
        let id = EventId(self.events.len());
        self.poll_set.add_fd_write(id, fd);
        self.events.push(event);
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
    pub(super) fn event_loop(&mut self, process: &mut T) -> StopReason<T> {
        let mut event_queue = Vec::with_capacity(self.events.len());

        loop {
            // FIXME: maybe we shout return the IO error instead.
            if let Ok(ids) = self.poll_set.poll() {
                for EventId(id) in ids {
                    let event = self.events[id];
                    event_queue.push(event);
                }

                for event in event_queue.drain(..) {
                    process.on_event(event, self);

                    if let Some(reason) = self.status.take_exit() {
                        return StopReason::Exit(reason);
                    }
                }
            }

            if let Some(reason) = self.status.take_stop() {
                return reason;
            }
        }
    }
}
