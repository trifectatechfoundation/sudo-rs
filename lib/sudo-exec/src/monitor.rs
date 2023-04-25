use std::{
    io,
    os::fd::OwnedFd,
    process::{exit, Child, Command},
    time::Duration,
};

use signal_hook::{
    consts::*,
    iterator::{exfiltrator::WithOrigin, SignalsInfo},
    low_level::{
        emulate_default_handler,
        siginfo::{Cause, Origin, Process, Sent},
    },
};
use sudo_log::user_error;
use sudo_system::{getpgid, interface::ProcessId, kill, set_controlling_terminal, setpgid, setsid};

use crate::ExitReason;

pub(super) struct MonitorRelay {
    signals: SignalsInfo<WithOrigin>,
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

            Ok((
                SignalsInfo::<WithOrigin>::new(super::SIGNALS)?,
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
                signals,
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
            self.wait_command()?;

            // Then we check any pending signals that we received. Based on `mon_signal_cb`
            for info in self.signals.pending() {
                self.relay_signal(info);
            }
        }
    }

    fn wait_command(&mut self) -> io::Result<()> {
        if let Some(status) = self.command.try_wait()? {
            ExitReason::from_status(status).send(&self.tx)?;

            // Given that we overwrote the default handlers for all the signals, we musti
            // emulate them to handle the signal we just sent correctly.
            for info in self.signals.pending() {
                emulate_default_handler(info.signal)?;
            }

            exit(0);
        }

        Ok(())
    }

    fn relay_signal(&self, info: Origin) {
        let user_signaled = info.cause == Cause::Sent(Sent::User);
        match info.signal {
            // FIXME: check `mon_handle_sigchld`
            SIGCHLD => {}
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if user_signaled && self.is_self_terminating(info.process) => {}
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

    /// Decides if the signal sent by the `signaler` process is self-terminating.
    ///
    /// A signal is self-terminating if the PID of the `process`:
    /// - is the same PID of the command, or
    /// - is in the process group of the command and the command is the leader.
    fn is_self_terminating(&self, signaler: Option<Process>) -> bool {
        if let Some(signaler) = signaler {
            if signaler.pid != 0 {
                if signaler.pid == self.command_pid {
                    return true;
                }

                if let Ok(grp_leader) = getpgid(signaler.pid) {
                    if grp_leader == self.command_pgrp {
                        return true;
                    }
                } else {
                    user_error!("Could not fetch process group ID");
                }
            }
        }

        false
    }
}
