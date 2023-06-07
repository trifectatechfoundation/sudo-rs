use std::{
    io,
    os::fd::OwnedFd,
    process::{exit, Child, Command, ExitStatus},
    time::Duration,
};

use signal_hook::consts::*;
use sudo_log::user_error;
use sudo_system::{
    getpgid, interface::ProcessId, kill, set_controlling_terminal, setpgid, setsid,
    signal::SignalInfo,
};

use crate::{signal::SignalHandlers, ExitReason};

pub(super) struct MonitorRelay {
    signal_handlers: SignalHandlers,
    command_pid: ProcessId,
    command_pgrp: ProcessId,
    command: Child,
    _pty_follower: OwnedFd,
    tx: OwnedFd,
}

impl MonitorRelay {
    pub(super) fn new(
        mut command: Command,
        pty_follower: OwnedFd,
        tx: OwnedFd,
    ) -> io::Result<Self> {
        let result = Ok(()).and_then(|()| {
            // Create new terminal session.
            setsid()?;

            // Set the pty as the controlling terminal.
            set_controlling_terminal(&pty_follower)?;

            // spawn and exec to command
            let command = command.spawn()?;

            let command_pid = command.id() as ProcessId;

            // set the process group ID of the command to the command PID.
            let command_pgrp = command_pid;
            setpgid(command_pid, command_pgrp);

            let signal_handlers = SignalHandlers::new()?;

            Ok((
                signal_handlers,
                command_pid,
                command_pgrp,
                command,
                pty_follower,
            ))
        });

        if result.is_err() {
            ExitReason::Code(1).send(&tx)?;
        }

        result.map(
            |(signals, command_pid, command_pgrp, command, pty_follower)| Self {
                signal_handlers: signals,
                command_pid,
                command_pgrp,
                command,
                _pty_follower: pty_follower,
                tx,
            },
        )
    }

    /// FIXME: this should return `!` but it is not stable yet.
    pub(super) fn run(mut self) -> io::Result<std::convert::Infallible> {
        loop {
            // First we check if the command is finished
            // FIXME: This should be polled alongside the signal handlers instead.
            if let Some(status) = self.wait_command()? {
                ExitReason::from_status(status).send(&self.tx)?;

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

    fn wait_command(&mut self) -> io::Result<Option<ExitStatus>> {
        self.command.try_wait()
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
