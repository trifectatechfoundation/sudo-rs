use std::{io, ops::ControlFlow, os::fd::OwnedFd};

use signal_hook::consts::*;
use sudo_log::user_error;
use sudo_system::{getpgid, interface::ProcessId, kill, signal::SignalInfo};

use crate::{
    backchannel::{MonitorEvent, ParentBackchannel, ParentEvent},
    io_util::{retry_while_interrupted, was_interrupted},
    signal::SignalHandlers,
    ExitReason,
};

pub(super) struct PtyRelay {
    signal_handlers: SignalHandlers,
    monitor_pid: ProcessId,
    sudo_pid: ProcessId,
    command_pid: Option<ProcessId>,
    // FIXME: Look for `SFD_LEADER` occurences in `exec_pty` to decide what to do with the leader
    // side of the pty. It should be used to handle signals like `SIGWINCH` and `SIGCONT`.
    _pty_leader: OwnedFd,
    backchannel: ParentBackchannel,
}

impl PtyRelay {
    pub(super) fn new(
        monitor_pid: ProcessId,
        sudo_pid: ProcessId,
        pty_leader: OwnedFd,
        mut backchannel: ParentBackchannel,
    ) -> io::Result<Self> {
        let signal_handlers = SignalHandlers::new()?;

        retry_while_interrupted(|| backchannel.send(MonitorEvent::ExecCommand))?;

        Ok(Self {
            signal_handlers,
            monitor_pid,
            sudo_pid,
            command_pid: None,
            _pty_leader: pty_leader,
            backchannel,
        })
    }

    pub(super) fn run(mut self) -> io::Result<(ExitReason, impl FnOnce())> {
        loop {
            // First we check the backchannel for any status updates from the command or the
            // monitor.
            if let ControlFlow::Break(event) = self.check_backchannel() {
                let exit_reason = match event {
                    ParentEvent::CommandExit(code) => ExitReason::Code(code),
                    ParentEvent::CommandSignal(signal) => ExitReason::Signal(signal),
                    ParentEvent::IoError(raw) => return Err(io::Error::from_raw_os_error(raw)),
                    // We never break the event loop because of this event.
                    ParentEvent::CommandPid(_) => unreachable!(),
                };

                return Ok((exit_reason, move || drop(self.signal_handlers)));
            }

            // Then we check any pending signals that we received. Based on `signal_cb_pty`
            if let Ok(infos) = self.signal_handlers.poll() {
                for info in infos {
                    self.relay_signal(info);
                }
            }
        }
    }

    /// Read an event from the backchannel and return the event if it should break the event loop.
    fn check_backchannel(&mut self) -> ControlFlow<ParentEvent> {
        match self.backchannel.recv() {
            // Not an actual error, we can retry later.
            Err(err) if was_interrupted(&err) => {}
            // Failed to read command status. This means that something is wrong with the socket
            // and we should stop.
            Err(err) => {
                return ControlFlow::Break((&err).into());
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
                    return ControlFlow::Break(event);
                }
            },
        }
        ControlFlow::Continue(())
    }

    fn relay_signal(&self, info: SignalInfo) {
        match info.signal() {
            // FIXME: check `handle_sigchld_pty`
            SIGCHLD => {}
            // FIXME: check `resume_terminal`
            SIGCONT => {}
            // FIXME: check `sync_ttysize`
            SIGWINCH => {}
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if info.is_user_signaled() && self.is_self_terminating(info.pid()) => {}
            // FIXME: check `send_command_status`
            signal => {
                kill(self.monitor_pid, signal).ok();
            }
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
}
