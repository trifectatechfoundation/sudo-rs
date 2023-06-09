use std::{
    io,
    os::fd::OwnedFd,
    process::{exit, Child, Command},
    time::Duration,
};

use signal_hook::consts::*;
use sudo_log::user_error;
use sudo_system::{
    getpgid, interface::ProcessId, kill, set_controlling_terminal, setpgid, setsid,
    signal::SignalInfo,
};

use crate::{
    backchannel::{MonitorBackchannel, MonitorEvent, ParentEvent},
    event::{EventClosure, EventHandler},
    io_util::retry_while_interrupted,
};

pub(super) struct MonitorRelay {
    command_pid: ProcessId,
    command_pgrp: ProcessId,
    command: Child,
    _pty_follower: OwnedFd,
    backchannel: MonitorBackchannel,
}

impl MonitorRelay {
    pub(super) fn new(
        mut command: Command,
        pty_follower: OwnedFd,
        mut backchannel: MonitorBackchannel,
    ) -> (Self, EventHandler<Self>) {
        let result = io::Result::Ok(()).and_then(|()| {
            let event_handler = EventHandler::<Self>::new()?;
            // Create new terminal session.
            setsid()?;

            // Set the pty as the controlling terminal.
            set_controlling_terminal(&pty_follower)?;

            // Wait for the main sudo process to give us green light before spawning the command. This
            // avoids race conditions when the command exits quickly.
            let MonitorEvent::ExecCommand = retry_while_interrupted(|| backchannel.recv())?;

            // spawn the command
            let command = command.spawn()?;

            let command_pid = command.id() as ProcessId;

            // Send the command's PID to the main sudo process.
            backchannel.send(ParentEvent::CommandPid(command_pid)).ok();

            // set the process group ID of the command to the command PID.
            let command_pgrp = command_pid;
            setpgid(command_pid, command_pgrp);

            Ok((
                event_handler,
                command_pid,
                command_pgrp,
                command,
                pty_follower,
            ))
        });

        match result {
            Err(err) => {
                backchannel.send((&err).into()).unwrap();
                exit(1);
            }
            Ok((event_handler, command_pid, command_pgrp, command, pty_follower)) => (
                Self {
                    command_pid,
                    command_pgrp,
                    command,
                    _pty_follower: pty_follower,
                    backchannel,
                },
                event_handler,
            ),
        }
    }

    pub(super) fn run(mut self, event_handler: &mut EventHandler<Self>) -> ! {
        let () = event_handler.event_loop(&mut self);
        drop(self);
        exit(0);
    }

    /// Decides if the signal sent by the process with `signaler_pid` PID is self-terminating.
    ///
    /// A signal is self-terminating if `signaler_pid`:
    /// - is the same PID of the command, or
    /// - is in the process group of the command and the command is the leader.
    fn is_self_terminating(&self, signaler_pid: ProcessId) -> bool {
        if signaler_pid != 0 {
            if signaler_pid == self.command_pid {
                return true;
            }

            if let Ok(grp_leader) = getpgid(signaler_pid) {
                if grp_leader == self.command_pgrp {
                    return true;
                }
            } else {
                user_error!("Could not fetch process group ID");
            }
        }

        false
    }
}

impl EventClosure for MonitorRelay {
    type Break = ();

    fn on_signal(&mut self, info: SignalInfo, event_handler: &mut EventHandler<Self>) {
        match info.signal() {
            // FIXME: check `mon_handle_sigchld`
            SIGCHLD => {
                if let Ok(Some(exit_status)) = self.command.try_wait() {
                    event_handler.set_break(());
                    self.backchannel.send(exit_status.into()).unwrap();
                }
            }
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if info.is_user_signaled() && self.is_self_terminating(info.pid()) => {}
            // Kill the command with increasing urgency.
            SIGALRM => {
                // Based on `terminate_command`.
                kill(self.command_pid, SIGHUP).ok();
                kill(self.command_pid, SIGTERM).ok();
                std::thread::sleep(Duration::from_secs(2));
                kill(self.command_pid, SIGKILL).ok();
            }
            signal => {
                kill(self.command_pid, signal).ok();
            }
        }
    }
}
