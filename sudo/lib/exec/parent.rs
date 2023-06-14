use std::collections::VecDeque;
use std::ffi::c_int;
use std::io;
use std::process::{exit, Command};

use signal_hook::consts::*;

use crate::log::user_error;
use crate::system::fork;
use crate::system::signal::{SignalAction, SignalHandler};
use crate::system::term::openpty;
use crate::system::{getpgid, interface::ProcessId, signal::SignalInfo};

use super::event::{EventClosure, EventDispatcher};
use super::monitor::exec_monitor;
use super::{
    backchannel::{BackchannelPair, MonitorMessage, ParentBackchannel, ParentMessage},
    io_util::{retry_while_interrupted, was_interrupted},
    ExitReason,
};

pub(super) fn exec_pty(
    sudo_pid: ProcessId,
    command: Command,
) -> io::Result<(ExitReason, impl FnOnce())> {
    // Allocate a pseudoterminal.
    // FIXME (ogsudo): We also need to open `/dev/tty` and set the right owner of the
    // pseudoterminal.
    let (pty_leader, pty_follower) = openpty()?;

    // Create backchannels to communicate with the monitor.
    let mut backchannels = BackchannelPair::new()?;

    // We don't want to receive SIGTTIN/SIGTTOU
    // FIXME: why?
    SignalHandler::with_action(SIGTTIN, SignalAction::Ignore).ok();
    SignalHandler::with_action(SIGTTOU, SignalAction::Ignore).ok();

    // FIXME (ogsudo): Initialize the policy plugin's session here by calling
    // `policy_init_session`.
    // FIXME (ogsudo): initializes ttyblock sigset here by calling `init_ttyblock`
    // FIXME (ogsudo): Set all the IO streams for the command to the follower side of the pty.
    // FIXME (ogsudo): Read from `/dev/tty` and write to the leader if not in the background.
    // FIXME (ogsudo): Read from the leader and write to `/dev/tty`.
    // FIXME (ogsudo): Do some extra setup if any of the IO streams are not a tty and logging is
    // enabled or if sudo is running in background.
    // FIXME (ogsudo): Copy terminal settings from `/dev/tty` to the pty.
    // FIXME (ogsudo): Start in raw mode unless we're part of a pipeline
    let mut dispatcher = EventDispatcher::<ParentClosure>::new()?;

    let monitor_pid = fork()?;

    if monitor_pid == 0 {
        // Close the file descriptors that we don't access
        drop(pty_leader);
        drop(backchannels.parent);

        // If `exec_monitor` returns, it means we failed to execute the command somehow.
        if let Err(err) = exec_monitor(pty_follower, command, &mut backchannels.monitor) {
            backchannels.monitor.send(&err.into()).ok();
        }
        // FIXME: drop everything before calling `exit`.
        exit(1)
    }

    // Close the file descriptors that we don't access
    drop(pty_follower);
    drop(backchannels.monitor);

    // Send green light to the monitor after closing the follower.
    retry_while_interrupted(|| backchannels.parent.send(&MonitorMessage::ExecCommand))?;

    let closure = ParentClosure::new(monitor_pid, sudo_pid, backchannels.parent, &mut dispatcher);

    // FIXME (ogsudo): Restore the signal handlers here.

    // FIXME (ogsudo): Retry if `/dev/tty` is revoked.
    closure
        .run(&mut dispatcher)
        .map(|exit_reason| (exit_reason, move || drop(dispatcher)))
}

struct ParentClosure {
    _monitor_pid: ProcessId,
    sudo_pid: ProcessId,
    command_pid: Option<ProcessId>,
    backchannel: ParentBackchannel,
    message_queue: VecDeque<MonitorMessage>,
}

impl ParentClosure {
    fn new(
        monitor_pid: ProcessId,
        sudo_pid: ProcessId,
        backchannel: ParentBackchannel,
        dispatcher: &mut EventDispatcher<Self>,
    ) -> Self {
        dispatcher.set_read_callback(&backchannel, |parent, dispatcher| {
            parent.on_message_received(dispatcher)
        });

        // Check for queued messages only when the backchannel can be written so we can send
        // messages to the monitor process without blocking.
        dispatcher.set_write_callback(&backchannel, |parent, dispatcher| {
            parent.check_message_queue(dispatcher)
        });

        Self {
            _monitor_pid: monitor_pid,
            sudo_pid,
            command_pid: None,
            backchannel,
            message_queue: VecDeque::new(),
        }
    }

    fn run(mut self, dispatcher: &mut EventDispatcher<Self>) -> io::Result<ExitReason> {
        let exit_reason = match dispatcher.event_loop(&mut self) {
            ParentMessage::IoError(code) => return Err(io::Error::from_raw_os_error(code)),
            ParentMessage::CommandExit(code) => ExitReason::Code(code),
            ParentMessage::CommandSignal(signal) => ExitReason::Signal(signal),
            // We never set this event as the last event
            ParentMessage::CommandPid(_) => unreachable!(),
        };

        Ok(exit_reason)
    }

    /// Read an event from the backchannel and return the event if it should break the event loop.
    fn on_message_received(&mut self, dispatcher: &mut EventDispatcher<Self>) {
        match self.backchannel.recv() {
            // Not an actual error, we can retry later.
            Err(err) if was_interrupted(&err) => {}
            // Failed to read command status. This means that something is wrong with the socket
            // and we should stop.
            Err(err) => {
                if !dispatcher.got_break() {
                    dispatcher.set_break(err.into());
                }
            }
            Ok(event) => match event {
                // Received the PID of the command. This means that the command is already
                // executing.
                ParentMessage::CommandPid(pid) => self.command_pid = pid.into(),
                // The command terminated or the monitor was not able to spawn it. We should stop
                // either way.
                ParentMessage::CommandExit(_)
                | ParentMessage::CommandSignal(_)
                | ParentMessage::IoError(_) => {
                    dispatcher.set_break(event);
                }
            },
        }
    }

    /// Decides if the signal sent by the process with `signaler_pid` PID is self-terminating.
    ///
    /// A signal is self-terminating if `signaler_pid`:
    /// - is the same PID of the command, or
    /// - is in the process group of the command and either sudo or the command is the leader.
    fn is_self_terminating(&self, signaler_pid: ProcessId) -> bool {
        if signaler_pid != 0 {
            if Some(signaler_pid) == self.command_pid {
                return true;
            }

            if let Ok(signaler_pgrp) = getpgid(signaler_pid) {
                if Some(signaler_pgrp) == self.command_pid || signaler_pgrp == self.sudo_pid {
                    return true;
                }
            } else {
                user_error!("Could not fetch process group ID");
            }
        }

        false
    }

    /// Schedule sending a signal event to the monitor using the backchannel.
    ///
    /// The signal message will be sent once the backchannel is ready to be written.
    fn schedule_signal(&mut self, signal: c_int) {
        self.message_queue.push_back(MonitorMessage::Signal(signal));
    }

    /// Send the first message in the event queue using the backchannel, if any.
    ///
    /// Calling this function will block until the backchannel can be written.
    fn check_message_queue(&mut self, dispatcher: &mut EventDispatcher<Self>) {
        if let Some(event) = self.message_queue.front() {
            match self.backchannel.send(event) {
                // The event was sent, remove it from the queue
                Ok(()) => {
                    self.message_queue.pop_front().unwrap();
                }
                // The other end of the socket is gone, we should exit.
                Err(err) if err.kind() == io::ErrorKind::BrokenPipe => {
                    // FIXME: maybe we need a different event for backchannel errors.
                    dispatcher.set_break(err.into());
                }
                // Non critical error, we can retry later.
                Err(_) => {}
            }
        }
    }
}

impl EventClosure for ParentClosure {
    type Break = ParentMessage;

    fn on_signal(&mut self, info: SignalInfo, dispatcher: &mut EventDispatcher<Self>) {
        match info.signal() {
            // FIXME: check `handle_sigchld_pty`
            SIGCHLD => self.on_message_received(dispatcher),
            // FIXME: check `resume_terminal`
            SIGCONT => {}
            // FIXME: check `sync_ttysize`
            SIGWINCH => {}
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if info.is_user_signaled() && self.is_self_terminating(info.pid()) => {}
            // FIXME: check `send_command_status`
            signal => self.schedule_signal(signal),
        }
    }
}
