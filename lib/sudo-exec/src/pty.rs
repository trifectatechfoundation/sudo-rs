use std::{ffi::c_int, io, os::fd::OwnedFd};

use signal_hook::{
    consts::*,
    flag::register_conditional_default,
    iterator::{exfiltrator::WithOrigin, SignalsInfo},
    low_level::{
        siginfo::{Cause, Origin, Process, Sent},
        signal_name,
    },
};
use sudo_log::{user_debug, user_error};
use sudo_system::{
    getpgid, interface::ProcessId, kill, killpg, waitpid, WaitError, WaitOptions, WaitStatus,
};

use crate::{
    log_signal, log_wait_status, socket::PtySocket, terminate_command, EmulateDefaultHandler,
    ExitReason, SIGCONT_FG,
};

pub(super) struct PtyRelay {
    signals: SignalsInfo<WithOrigin>,
    monitor_pid: Option<ProcessId>,
    sudo_pid: ProcessId,
    command_pid: Option<ProcessId>,
    parent_pgrp: ProcessId,
    // FIXME: Look for `SFD_LEADER` occurences in `exec_pty` to decide what to do with the leader
    // side of the pty. It should be used to handle signals like `SIGWINCH` and `SIGCONT`.
    _pty_leader: OwnedFd,
    pty_follower: OwnedFd,
    socket: PtySocket,
}

impl PtyRelay {
    pub(super) fn new(
        monitor_pid: ProcessId,
        sudo_pid: ProcessId,
        parent_pgrp: ProcessId,
        pty_leader: OwnedFd,
        pty_follower: OwnedFd,
        socket: PtySocket,
    ) -> io::Result<Self> {
        Ok(Self {
            signals: SignalsInfo::<WithOrigin>::new(super::SIGNALS)?,
            monitor_pid: Some(monitor_pid),
            sudo_pid,
            command_pid: None,
            parent_pgrp,
            _pty_leader: pty_leader,
            pty_follower,
            socket,
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
            if let Ok(Some(exit_reason)) = self.handle_status() {
                return Ok((exit_reason, emulate_default_handler));
            }

            // Then we check any pending signals that we received. Based on `signal_cb_pty`
            for info in self.signals.wait() {
                self.relay_signal(info);
            }
        }
    }

    fn handle_status(&mut self) -> io::Result<Option<ExitReason>> {
        let status = self.socket.receive_status()?;

        if let Some(pid) = status.command_pid() {
            user_debug!("received command PID {pid} from monitor");
            self.command_pid = Some(pid);
        } else if let Some(raw) = status.wait() {
            user_debug!("received command wait status {raw} from monitor");
            let wait_status = WaitStatus::from_raw(raw);
            if let Some(signal) = wait_status.stopped() {
                self.suspend(signal);
                self.socket.send_signal(signal).ok();
            } else if let Some(signal) = wait_status.signaled() {
                return Ok(Some(ExitReason::Signal(signal)));
            } else if let Some(code) = wait_status.exit_status() {
                return Ok(Some(ExitReason::Code(code)));
            }
        } else if let Some(raw) = status.monitor_err() {
            let err = io::Error::from_raw_os_error(raw);
            return Ok(Some(ExitReason::Code(1)));
        }

        Ok(None)
    }

    fn relay_signal(&mut self, info: Origin) {
        log_signal(&info, "pty");
        let user_signaled = info.cause == Cause::Sent(Sent::User);
        match info.signal {
            // FIXME: check `handle_sigchld_pty`
            SIGCHLD => self.handle_sigchld(),
            // FIXME: check `resume_terminal`
            SIGCONT => {}
            // FIXME: check `sync_ttysize`
            SIGWINCH => {}
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if user_signaled && self.is_self_terminating(info.process) => {}
            signal => {
                user_debug!(
                    "pty sending {} to monitor over socket",
                    signal_name(signal).unwrap_or("unknown signal")
                );
                self.socket.send_signal(signal).ok();
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
                if Some(signaler.pid) == self.command_pid {
                    return true;
                }

                if let Ok(signaler_pgrp) = getpgid(signaler.pid) {
                    if Some(signaler_pgrp) == self.command_pid || signaler_pgrp == self.sudo_pid {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn handle_sigchld(&mut self) {
        loop {
            let status = loop {
                match waitpid(None, WaitOptions::default().all().untraced().no_hang()) {
                    Err(WaitError::Signal) => {}
                    Err(WaitError::Unavailable) => {
                        return user_debug!("pty's children are not available");
                    }
                    Err(WaitError::Io(err)) => {
                        return user_debug!("pty failed waiting for child: {}", err);
                    }
                    Ok(status) => {
                        log_wait_status(&status, "pty's child process");
                        break status;
                    }
                }
            };

            let pid = status.pid();
            if status.exit_status().is_some() || status.signaled().is_some() {
                if Some(pid) == self.monitor_pid {
                    self.monitor_pid = None;
                }
            } else if let Some(signal) = status.stopped() {
                if Some(pid) != self.monitor_pid {
                    continue;
                }
                let signal = self.suspend(signal);
                user_debug!("sending SIGCONT to {pid}");
                kill(pid, SIGCONT).ok();
                self.socket.send_signal(signal).ok();
            }
        }
    }

    fn suspend(&mut self, signal: c_int) -> c_int {
        let ret;
        // FIXME: ignore SIGCONT once `resume_terminal` has been implemented.
        match signal {
            SIGTTOU | SIGTTIN => {
                ret = SIGCONT_FG;
            }
            SIGSTOP | SIGTSTP | _ => {
                if signal != SIGSTOP {
                    // FIXME: change handler
                }

                if (self.parent_pgrp != self.sudo_pid && kill(self.parent_pgrp, 0).is_err())
                    || killpg(self.parent_pgrp, signal).is_err()
                {
                    terminate_command(self.command_pid, true);
                    self.command_pid = None;
                }

                if signal != SIGSTOP {
                    // FIXME: restore handler
                }

                ret = SIGCONT_FG;
            }
        }

        // FIXME: restore SIGCONT handler.

        ret
    }
}
