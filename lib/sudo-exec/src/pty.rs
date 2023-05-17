use std::{io, os::fd::OwnedFd};

use signal_hook::{
    consts::*,
    flag::register_conditional_default,
    iterator::{exfiltrator::WithOrigin, SignalsInfo},
    low_level::siginfo::{Cause, Origin, Process, Sent},
};
use sudo_log::user_error;
use sudo_system::{getpgid, interface::ProcessId, kill};

use crate::{EmulateDefaultHandler, ExitReason};

pub(super) struct PtyRelay {
    signals: SignalsInfo<WithOrigin>,
    monitor_pid: ProcessId,
    sudo_pid: ProcessId,
    command_pid: ProcessId,
    // FIXME: Look for `SFD_LEADER` occurences in `exec_pty` to decide what to do with the leader
    // side of the pty. It should be used to handle signals like `SIGWINCH` and `SIGCONT`.
    _pty_leader: OwnedFd,
    rx: OwnedFd,
}

impl PtyRelay {
    pub(super) fn new(
        monitor_pid: ProcessId,
        sudo_pid: ProcessId,
        pty_leader: OwnedFd,
        rx: OwnedFd,
    ) -> io::Result<Self> {
        Ok(Self {
            signals: SignalsInfo::<WithOrigin>::new(super::SIGNALS)?,
            monitor_pid,
            sudo_pid,
            // FIXME: is this ok? Check ogsudo's code.
            command_pid: -1,
            _pty_leader: pty_leader,
            rx,
        })
    }

    pub(super) fn run(mut self) -> io::Result<(ExitReason, EmulateDefaultHandler)> {
        let emulate_default_handler = EmulateDefaultHandler::default();

        for &signal in super::SIGNALS {
            register_conditional_default(
                signal,
                EmulateDefaultHandler::clone(&emulate_default_handler),
            )?;
        }

        loop {
            // First we check if the monitor sent us the exit status of the command.
            if let Ok(reason) = self.wait_monitor() {
                return Ok((reason, emulate_default_handler));
            }

            // Then we check any pending signals that we received. Based on `signal_cb_pty`
            for info in self.signals.wait() {
                self.relay_signal(info);
            }
        }
    }

    fn wait_monitor(&mut self) -> io::Result<ExitReason> {
        ExitReason::recv(&self.rx)
    }

    fn relay_signal(&self, info: Origin) {
        let user_signaled = info.cause == Cause::Sent(Sent::User);
        match info.signal {
            // FIXME: check `handle_sigchld_pty`
            SIGCHLD => {}
            // FIXME: check `resume_terminal`
            SIGCONT => {}
            // FIXME: check `sync_ttysize`
            SIGWINCH => {}
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if user_signaled && self.is_self_terminating(info.process) => {}
            // FIXME: check `send_command_status`
            signal => {
                kill(self.monitor_pid, signal).ok();
            }
        }
    }

    /// Decides if the signal sent by the `signaler` process is self-terminating.
    ///
    /// A signal is self-terminating if the PID of the `process`:
    /// - is the same PID of the command, or
    /// - is in the process group of the command and either sudo or the command is the leader.
    fn is_self_terminating(&self, signaler: Option<Process>) -> bool {
        if let Some(signaler) = signaler {
            if signaler.pid != 0 {
                if signaler.pid == self.command_pid {
                    return true;
                }

                if let Ok(signaler_pgrp) = getpgid(signaler.pid) {
                    if signaler_pgrp == self.command_pid || signaler_pgrp == self.sudo_pid {
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
