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
    io_util::retry_while_interrupted,
    signal::SignalHandlers,
};

pub(super) struct MonitorRelay {
    signal_handlers: SignalHandlers,
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
    ) -> io::Result<Self> {
        let result = io::Result::Ok(()).and_then(|()| {
            let signal_handlers = SignalHandlers::new()?;

            // Create new terminal session.
            setsid()?;

            // Set the pty as the controlling terminal.
            set_controlling_terminal(&pty_follower)?;

            // Wait for the main sudo process to give us green light before spawning the command. This
            // avoids race conditions when the command exits quickly.
            let MonitorEvent::ExecCommand = retry_while_interrupted(|| backchannel.recv())?;

            // spawn and exec to command
            let command = command.spawn()?;

            let command_pid = command.id() as ProcessId;

            // Send the command's PID to the main sudo process.
            backchannel.send(ParentEvent::CommandPid(command_pid)).ok();

            // set the process group ID of the command to the command PID.
            let command_pgrp = command_pid;
            setpgid(command_pid, command_pgrp);

            Ok((
                signal_handlers,
                command_pid,
                command_pgrp,
                command,
                pty_follower,
            ))
        });

        match result {
            Err(err) => {
                backchannel.send((&err).into())?;
                Err(err)
            }
            Ok((signals, command_pid, command_pgrp, command, pty_follower)) => Ok(Self {
                signal_handlers: signals,
                command_pid,
                command_pgrp,
                command,
                _pty_follower: pty_follower,
                backchannel,
            }),
        }
    }

    pub(super) fn run(mut self) -> ! {
        loop {
            // First we check if the command is finished
            // FIXME: This should be polled alongside the signal handlers instead.
            if let Ok(Some(status)) = self.command.try_wait() {
                self.backchannel.send(status.into()).ok();

                exit(0);
            }

            // Then we check any pending signals that we received. Based on `mon_signal_cb`.
            //
            // Right now, we rely on the fact that `poll` can be interrupted by a signal so this
            // call doesn't block forever. We are guaranteed to receive `SIGCHLD` when the
            // command terminates meaning that the `wait_command` call above will succeed on the
            // next iteration of this loop. We won't have to rely on this behavior once we
            // integrate `wait_command` into the `poll` itself.
            if let Ok(infos) = self.signal_handlers.poll() {
                for info in infos {
                    self.relay_signal(info);
                }
            }
        }
    }

    fn relay_signal(&self, info: SignalInfo) {
        match info.signal() {
            // FIXME: check `mon_handle_sigchld`
            SIGCHLD => {}
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
