use std::{io, process::Command};

use signal_hook::consts::*;

use super::{
    event::{EventClosure, EventDispatcher, StopReason},
    terminate_process, ExitReason,
};
use crate::{
    exec::io_util::was_interrupted,
    system::{
        getpgid,
        interface::ProcessId,
        kill,
        signal::SignalInfo,
        wait::{waitpid, WaitError, WaitOptions},
    },
};

pub(crate) fn exec_no_pty(
    sudo_pid: ProcessId,
    mut command: Command,
) -> io::Result<(ExitReason, Box<dyn FnOnce()>)> {
    // FIXME (ogsudo): Initialize the policy plugin's session here.

    // FIXME: block signals directly instead of using the dispatcher.
    let mut dispatcher = EventDispatcher::new()?;

    // FIXME (ogsudo): Some extra config happens here if selinux is available.

    let command = command.spawn()?;

    let mut closure = ExecClosure::new(command.id() as ProcessId, sudo_pid);

    // FIXME: restore signal mask here.

    let exit_reason = match dispatcher.event_loop(&mut closure) {
        StopReason::Break(reason) => match reason {},
        StopReason::Exit(reason) => reason,
    };

    Ok((exit_reason, Box::new(move || drop(dispatcher))))
}

struct ExecClosure {
    command_pid: Option<ProcessId>,
    sudo_pid: ProcessId,
}

impl ExecClosure {
    fn new(command_pid: ProcessId, sudo_pid: ProcessId) -> Self {
        Self {
            command_pid: Some(command_pid),
            sudo_pid,
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
            }
        }

        false
    }

    fn handle_sigchld(&mut self, command_pid: ProcessId, dispatcher: &mut EventDispatcher<Self>) {
        const OPTS: WaitOptions = WaitOptions::new().all().untraced().no_hang();

        let status = loop {
            match waitpid(command_pid, OPTS) {
                Err(WaitError::Io(err)) if was_interrupted(&err) => {}
                Err(_) => {}
                Ok((_pid, status)) => break status,
            }
        };

        if let Some(_signal) = status.stop_signal() {
            // FIXME: check `sudo_suspend_parent`
        } else if let Some(signal) = status.term_signal() {
            dispatcher.set_exit(ExitReason::Signal(signal));
            self.command_pid = None;
        } else if let Some(exit_code) = status.exit_status() {
            dispatcher.set_exit(ExitReason::Code(exit_code));
            self.command_pid = None;
        }
    }
}

impl EventClosure for ExecClosure {
    type Break = std::convert::Infallible;
    type Exit = ExitReason;

    fn on_signal(&mut self, info: SignalInfo, dispatcher: &mut EventDispatcher<Self>) {
        let Some(command_pid) = self.command_pid else {
            return;
        };

        match info.signal() {
            SIGCHLD => {
                self.handle_sigchld(command_pid, dispatcher);
            }
            signal => {
                if signal == SIGWINCH {
                    // FIXME: check `handle_sigwinch_no_pty`.
                }
                // Skip the signal if it was sent by the user and it is self-terminating.
                if info.is_user_signaled() && self.is_self_terminating(info.pid()) {
                    return;
                }

                if signal == SIGALRM {
                    terminate_process(command_pid, false);
                } else {
                    kill(command_pid, signal).ok();
                }
            }
        }
    }
}
