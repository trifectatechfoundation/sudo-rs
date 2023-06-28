use std::{collections::BTreeMap, fmt::Debug, os::fd::AsRawFd};

use crate::{
    log::dev_debug,
    system::poll::{PollEvent, PollSet},
};

pub(super) trait Process: Sized {
    /// IO Events that this process should handle.
    type Event: Copy + Eq + Debug;
    /// Reason why the event loop should break.
    ///
    /// See [`EventRegistry::set_break`] for more information.
    type Break;
    /// Reason why the event loop should exit.
    ///
    /// See [`EventRegistry::set_exit`] for more information.
    type Exit;
    /// Handle the corresponding event.
    fn on_event(&mut self, event: Self::Event, registry: &mut EventRegistry<Self>);
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

#[derive(PartialEq, Eq, Hash, Ord, PartialOrd, Clone, Copy)]
struct EventId(usize);

pub(super) struct EventHandle {
    id: EventId,
    should_poll: bool,
}

impl EventHandle {
    /// Ignore the event associated with this handle, meaning that the file descriptor for this
    /// event will not be polled anymore for that specific event.
    pub(super) fn ignore<T: Process>(&mut self, registry: &mut EventRegistry<T>) {
        if self.should_poll {
            registry.poll_set.ignore_fd(self.id);
            self.should_poll = false;
        }
    }

    /// Stop ignoring the event associated with this handle, meaning that the file descriptor for
    /// this event will be polled for that specific event.
    pub(super) fn resume<T: Process>(&mut self, registry: &mut EventRegistry<T>) {
        if !self.should_poll {
            registry.poll_set.resume_fd(self.id);
            self.should_poll = true;
        }
    }
}

/// A type able to register file descriptors to be polled.
pub(super) struct EventRegistry<T: Process> {
    seed: usize,
    poll_set: PollSet<EventId>,
    events: BTreeMap<EventId, T::Event>,
    status: Status<T>,
}

impl<T: Process> EventRegistry<T> {
    /// Create a new and empty registry..
    pub(super) fn new() -> Self {
        Self {
            seed: 0,
            poll_set: PollSet::new(),
            events: BTreeMap::new(),
            status: Status::Continue,
        }
    }

    fn next_id(&mut self) -> EventId {
        let id = EventId(self.seed);
        self.seed += 1;
        id
    }

    /// Set the `fd` descriptor to be polled for `poll_event` events and produce `event` when `fd` is
    /// ready.
    pub(super) fn register_event<F: AsRawFd>(
        &mut self,
        fd: &F,
        poll_event: PollEvent,
        event_fn: impl Fn(PollEvent) -> T::Event,
    ) -> EventHandle {
        let id = self.next_id();
        self.poll_set.add_fd(id, fd, poll_event);
        self.events.insert(id, event_fn(poll_event));
        EventHandle {
            id,
            should_poll: true,
        }
    }

    /// Stop the event loop when the current event has been handled and set a reason for it.
    ///
    /// This means that the event loop will stop even if other events are ready.
    pub(super) fn set_break(&mut self, reason: T::Break) {
        self.status = Status::Stop(StopReason::Break(reason));
    }

    /// Stop the event loop when the events that are ready by now have been handled and set a
    /// reason for it.
    pub(super) fn set_exit(&mut self, reason: T::Exit) {
        self.status = Status::Stop(StopReason::Exit(reason));
    }

    /// Return whether a break reason has been set already. This function will return `false` after
    /// [`EventRegistry::event_loop`] has been called.
    pub(super) fn got_break(&self) -> bool {
        self.status.is_break()
    }

    /// Run the event loop over this registry using `process` to handle the produced events.
    ///
    /// The event loop will continue indefinitely unless you call [`EventRegistry::set_break`] or
    /// [`EventRegistry::set_exit`].
    #[track_caller]
    pub(super) fn event_loop(&mut self, process: &mut T) -> StopReason<T> {
        let mut event_queue = Vec::with_capacity(self.events.len());

        loop {
            // FIXME: maybe we shout return the IO error instead.
            if let Ok(ids) = self.poll_set.poll() {
                for id in ids {
                    let event = self.events[&id];
                    dev_debug!("event {event:?} is ready");
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
