use std::{
    ffi::c_int,
    io,
    os::{fd::OwnedFd, unix::process::CommandExt},
    process::{exit, Command},
};

use signal_hook::{
    consts::*,
    iterator::{exfiltrator::WithOrigin, SignalsInfo},
    low_level::{
        emulate_default_handler,
        siginfo::{Cause, Origin, Process, Sent},
        signal_name,
    },
};
use sudo_log::{user_debug, user_error};
use sudo_system::interface::ProcessId;
use sudo_system::{
    getpgid, getpgrp, killpg, set_controlling_terminal, setpgid, setsid, tcsetpgrp, waitpid,
    WaitError, WaitOptions,
};

use crate::{
    log_signal, log_wait_status,
    socket::{CommandStatus, MonitorSocket},
    terminate_command, SIGCONT_BG, SIGCONT_FG,
};

pub(super) struct MonitorRelay {
    signals: SignalsInfo<WithOrigin>,
    command_pid: Option<ProcessId>,
    command_pgrp: ProcessId,
    command_status: CommandStatus,
    monitor_pgrp: ProcessId,
    pty_follower: OwnedFd,
    socket: MonitorSocket,
}

impl MonitorRelay {
    pub(super) fn new(
        mut command: Command,
        pty_follower: OwnedFd,
        mut socket: MonitorSocket,
    ) -> io::Result<Self> {
        // Create new terminal session.
        setsid()?;

        // Set the pty as the controlling terminal.
        set_controlling_terminal(&pty_follower)?;

        // set the process group ID of the command to the command PID. This is done here too to
        // avoid any potential races where either the monitor or the command observe a
        // different process group ID for the command.
        #[allow(unsafe_code)]
        unsafe {
            command.pre_exec(|| {
                let pid = std::process::id() as ProcessId;
                setpgid(0, pid).ok();
                Ok(())
            });
        }

        // spawn and exec to command
        let command = command.spawn()?;

        let command_pid = command.id() as ProcessId;
        // send the command's PID to the main sudo process.
        socket.send_status(CommandStatus::from_pid(command_pid))?;

        let monitor_pgrp = getpgrp()?;

        // set the process group ID of the command to the command PID.
        let command_pgrp = command_pid;
        setpgid(command_pid, command_pgrp).ok();

        // set the command process group as the foreground process group for the pty follower.
        tcsetpgrp(&pty_follower, command_pgrp).ok();

        let signals = SignalsInfo::<WithOrigin>::new(super::SIGNALS)?;

        Ok(Self {
            signals,
            command_pid: Some(command_pid),
            command_pgrp,
            command_status: CommandStatus::default(),
            monitor_pgrp,
            pty_follower,
            socket,
        })
    }

    /// FIXME: this should return `!` but it is not stable yet.
    pub(super) fn run(mut self) -> io::Result<std::convert::Infallible> {
        loop {
            // First we check if the command is finished
            self.wait_command()?;

            if let Ok(signal) = self.socket.receive_signal() {
                user_debug!(
                    "monitor received {} via socket",
                    signal_name(signal).unwrap_or_else(|| match signal {
                        SIGCONT_FG => "SIGCONT_FG",
                        SIGCONT_BG => "SIGCONT_BG",
                        _ => "unknown signal",
                    })
                );
                self.handle_signal(signal);
            }

            // Then we check any pending signals that we received. Based on `mon_signal_cb`
            for info in self.signals.wait() {
                self.relay_signal(info);
            }
        }
    }

    fn wait_command(&mut self) -> io::Result<()> {
        if self.command_pid.is_none() {
            // emulate them to handle the signal we just sent correctly.
            for info in self.signals.pending() {
                emulate_default_handler(info.signal)?;
            }

            self.socket.send_status(self.command_status)?;
            user_debug!("monitor is done");
            exit(0);
        }

        Ok(())
    }

    fn relay_signal(&mut self, info: Origin) {
        log_signal(&info, "monitor");
        let user_signaled = info.cause == Cause::Sent(Sent::User);
        match info.signal {
            SIGCHLD => self.handle_sigchld(),
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if user_signaled && self.is_self_terminating(info.process) => {}
            signal => self.handle_signal(signal),
        }
    }

    /// Decides if the signal sent by the `signaler` process is self-terminating.
    ///
    /// A signal is self-terminating if the PID of the `signaler`:
    /// - is the same PID of the command, or
    /// - is in the process group of the command and the command is the leader.
    fn is_self_terminating(&self, signaler: Option<Process>) -> bool {
        if let Some(signaler) = signaler {
            if signaler.pid != 0 {
                if Some(signaler.pid) == self.command_pid {
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

    fn handle_sigchld(&mut self) {
        let status = loop {
            match waitpid(
                self.command_pid,
                WaitOptions::default().all().untraced().no_hang(),
            ) {
                Err(WaitError::Signal) => continue,
                Err(WaitError::Unavailable) => {
                    return user_debug!("command is not available");
                }
                Err(WaitError::Io(err)) => {
                    user_debug!("monitor failed waiting for command: {}", err);
                }
                Ok(status) => {
                    log_wait_status(&status, "command");
                    break status;
                }
            }
        };

        if self.command_status.is_invalid() {
            self.command_status = status.into();
            if status.stopped().is_some() {
                let pgid = sudo_system::tcgetpgrp(&self.pty_follower).unwrap();
                if pgid != self.monitor_pgrp {
                    self.command_pgrp = pgid;
                }
                user_debug!("command was stopped, sending status via socket");
                self.socket.send_status(self.command_status).ok();
                self.command_status = Default::default();
            } else if status.signaled().is_some() || status.exit_status().is_some() {
                self.command_pid = None;
            }
        } else {
            user_debug!(
                "not overwriting command status {:?} with {:?}",
                self.command_status,
                CommandStatus::from(status)
            )
        }
    }

    fn handle_signal(&mut self, signal: c_int) {
        let command_pid = match self.command_pid {
            Some(pid) => pid,
            None => return,
        };

        match signal {
            SIGALRM => terminate_command(Some(command_pid), true),
            SIGCONT_FG => {
                tcsetpgrp(&self.pty_follower, self.command_pgrp).ok();
                user_debug!("monitor sending SIGCONT to command");
                killpg(command_pid, SIGCONT).ok();
            }
            SIGCONT_BG => {
                tcsetpgrp(&self.pty_follower, self.monitor_pgrp).ok();
                user_debug!("monitor sending SIGCONT to command");
                killpg(command_pid, SIGCONT).ok();
            }
            _ => {
                user_debug!(
                    "monitor sending {} to command",
                    signal_name(signal).unwrap_or("unknown signal")
                );
                killpg(command_pid, signal).ok();
            }
        }
    }
}
