use std::ffi::c_int;
use std::{io, os::fd::OwnedFd};

use signal_hook::consts::*;
use sudo_log::user_error;
use sudo_system::{getpgid, interface::ProcessId, signal::SignalInfo};

use crate::event::{EventClosure, EventDispatcher};
use crate::{
    backchannel::{MonitorEvent, ParentBackchannel, ParentEvent},
    io_util::{retry_while_interrupted, was_interrupted},
    ExitReason,
};

pub(super) struct ParentClosure {
    _monitor_pid: ProcessId,
    sudo_pid: ProcessId,
    command_pid: Option<ProcessId>,
    // FIXME: Look for `SFD_LEADER` occurences in `exec_pty` to decide what to do with the leader
    // side of the pty. It should be used to handle signals like `SIGWINCH` and `SIGCONT`.
    _pty_leader: OwnedFd,
    backchannel: ParentBackchannel,
}

impl ParentClosure {
    pub(super) fn new(
        monitor_pid: ProcessId,
        sudo_pid: ProcessId,
        pty_leader: OwnedFd,
        mut backchannel: ParentBackchannel,
    ) -> io::Result<(Self, EventDispatcher<Self>)> {
        let mut dispatcher = EventDispatcher::<Self>::new()?;

        dispatcher.set_read_callback(&backchannel, |parent, dispatcher| {
            parent.check_backchannel(dispatcher)
        });

        retry_while_interrupted(|| backchannel.send(MonitorEvent::ExecCommand))?;

        Ok((
            Self {
                _monitor_pid: monitor_pid,
                sudo_pid,
                command_pid: None,
                _pty_leader: pty_leader,
                backchannel,
            },
            dispatcher,
        ))
    }

    pub(super) fn run(mut self, dispatcher: &mut EventDispatcher<Self>) -> io::Result<ExitReason> {
        let exit_reason = match dispatcher.event_loop(&mut self) {
            ParentEvent::IoError(code) => return Err(io::Error::from_raw_os_error(code)),
            ParentEvent::CommandExit(code) => ExitReason::Code(code),
            ParentEvent::CommandSignal(signal) => ExitReason::Signal(signal),
            // We never set this event as the last event
            ParentEvent::CommandPid(_) => unreachable!(),
        };

        Ok(exit_reason)
    }

    /// Read an event from the backchannel and return the event if it should break the event loop.
    fn check_backchannel(&mut self, dispatcher: &mut EventDispatcher<Self>) {
        match self.backchannel.recv() {
            // Not an actual error, we can retry later.
            Err(err) if was_interrupted(&err) => {}
            // Failed to read command status. This means that something is wrong with the socket
            // and we should stop.
            Err(err) => {
                if !dispatcher.got_break() {
                    dispatcher.set_break((&err).into());
                }
            }
            Ok(event) => match event {
                // Received the PID of the command. This means that the command is already
                // executing.
                ParentEvent::CommandPid(pid) => self.command_pid = pid.into(),
                // The command terminated or the monitor was not able to spawn it. We should stop
                // either way.
                ParentEvent::CommandExit(_)
                | ParentEvent::CommandSignal(_)
                | ParentEvent::IoError(_) => {
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

    /// Send a signal event to the monitor using the backchannel.
    fn send_signal(&mut self, signal: c_int) -> io::Result<()> {
        self.backchannel.send(MonitorEvent::Signal(signal))
    }
}

impl EventClosure for ParentClosure {
    type Break = ParentEvent;

    fn on_signal(&mut self, info: SignalInfo, dispatcher: &mut EventDispatcher<Self>) {
        match info.signal() {
            // FIXME: check `handle_sigchld_pty`
            SIGCHLD => self.check_backchannel(dispatcher),
            // FIXME: check `resume_terminal`
            SIGCONT => {}
            // FIXME: check `sync_ttysize`
            SIGWINCH => {}
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if info.is_user_signaled() && self.is_self_terminating(info.pid()) => {}
            // FIXME: check `send_command_status`
            signal => {
                self.send_signal(signal).ok();
            }
        }
    }
}
