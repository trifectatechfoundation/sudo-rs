use std::{io, process::Command};

use signal_hook::consts::*;

use super::{
    event::{EventRegistry, Process, StopReason},
    signal_manager::Signal,
    terminate_process, ExitReason,
};
use crate::{
    exec::{io_util::was_interrupted, opt_fmt, signal_fmt, signal_manager::SignalManager},
    log::{dev_error, dev_info, dev_warn},
    system::{
        getpgid, getpgrp,
        interface::ProcessId,
        kill, killpg,
        signal::{SignalAction, SignalNumber},
        term::{Terminal, UserTerm},
        wait::{Wait, WaitError, WaitOptions},
    },
};

pub(crate) fn exec_no_pty(
    sudo_pid: ProcessId,
    mut command: Command,
) -> io::Result<(ExitReason, Box<dyn FnOnce()>)> {
    // FIXME (ogsudo): Initialize the policy plugin's session here.

    // FIXME: block signals directly instead of using the manager.
    let signal_manager = SignalManager::new()?;

    // FIXME (ogsudo): Some extra config happens here if selinux is available.

    let command = command.spawn().map_err(|err| {
        dev_error!("cannot spawn command: {err}");
        err
    })?;

    let command_pid = command.id() as ProcessId;
    dev_info!("executed command with pid {command_pid}");

    let mut registry = EventRegistry::new();

    let mut closure = ExecClosure::new(command_pid, sudo_pid, signal_manager, &mut registry);

    // FIXME: restore signal mask here.

    let exit_reason = match registry.event_loop(&mut closure) {
        StopReason::Break(reason) => match reason {},
        StopReason::Exit(reason) => reason,
    };

    Ok((exit_reason, Box::new(move || drop(registry))))
}

struct ExecClosure {
    command_pid: Option<ProcessId>,
    sudo_pid: ProcessId,
    parent_pgrp: ProcessId,
    signal_manager: SignalManager,
}

impl ExecClosure {
    fn new(
        command_pid: ProcessId,
        sudo_pid: ProcessId,
        signal_manager: SignalManager,
        registry: &mut EventRegistry<Self>,
    ) -> Self {
        signal_manager.register_handlers(registry, ExecEvent::Signal);

        Self {
            command_pid: Some(command_pid),
            sudo_pid,
            parent_pgrp: getpgrp(),
            signal_manager,
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

    fn handle_sigchld(&mut self, command_pid: ProcessId, registry: &mut EventRegistry<Self>) {
        const OPTS: WaitOptions = WaitOptions::new().all().untraced().no_hang();

        let status = loop {
            match command_pid.wait(OPTS) {
                Err(WaitError::Io(err)) if was_interrupted(&err) => {}
                // This only happens if we receive `SIGCHLD` but there's no status update from the
                // command.
                Err(WaitError::Io(err)) => {
                    dev_info!("cannot wait for {command_pid} (command): {err}")
                }
                // This only happens if the monitor exited and any process already waited for the
                // command.
                Err(WaitError::NotReady) => {
                    dev_info!("{command_pid} (command) has no status report")
                }
                Ok((_pid, status)) => break status,
            }
        };

        if let Some(signal) = status.stop_signal() {
            dev_info!(
                "{command_pid} (command) was stopped by {}",
                signal_fmt(signal),
            );
            self.suspend_parent(signal);
        } else if let Some(signal) = status.term_signal() {
            dev_info!(
                "{command_pid} (command) was terminated by {}",
                signal_fmt(signal),
            );
            registry.set_exit(ExitReason::Signal(signal));
            self.command_pid = None;
        } else if let Some(exit_code) = status.exit_status() {
            dev_info!("{command_pid} (command) exited with status code {exit_code}");
            registry.set_exit(ExitReason::Code(exit_code));
            self.command_pid = None;
        } else if status.did_continue() {
            dev_info!("{command_pid} (command) continued execution");
        } else {
            dev_warn!("unexpected wait status for {command_pid} (command)")
        }
    }

    /// Suspend the main process.
    fn suspend_parent(&self, signal: SignalNumber) {
        let mut opt_tty = UserTerm::open().ok();
        let mut opt_pgrp = None;

        if let Some(tty) = opt_tty.as_ref() {
            if let Ok(saved_pgrp) = tty.tcgetpgrp() {
                // Save the terminal's foreground process group so we can restore it after resuming
                // if needed.
                opt_pgrp = Some(saved_pgrp);
            } else {
                opt_tty.take();
            }
        }

        if let Some(saved_pgrp) = opt_pgrp {
            // This means that the command was stopped trying to access the terminal. If the
            // terminal has a different foreground process group and we own the terminal, we give
            // it to the command and let it continue.
            if let SIGTTOU | SIGTTIN = signal {
                if saved_pgrp == self.parent_pgrp {
                    if let Some(command_pgrp) = self.command_pid.and_then(|pid| getpgid(pid).ok()) {
                        if command_pgrp != self.parent_pgrp
                            && opt_tty
                                .as_ref()
                                .is_some_and(|tty| tty.tcsetpgrp_nobg(command_pgrp).is_ok())
                        {
                            if let Err(err) = killpg(command_pgrp, SIGCONT) {
                                dev_warn!("cannot send SIGCONT to command ({command_pgrp}): {err}");
                            }

                            return;
                        }
                    }
                }
            }
        }

        let sigtstp_action = (signal == SIGTSTP).then(|| {
            self.signal_manager
                .set_action(Signal::SIGTSTP, SignalAction::Default)
        });

        if let Err(err) = kill(self.sudo_pid, signal) {
            dev_warn!(
                "cannot send {} to {} (sudo): {err}",
                signal_fmt(signal),
                self.sudo_pid
            );
        }

        if let Some(action) = sigtstp_action {
            self.signal_manager.set_action(Signal::SIGTSTP, action);
        }

        if let Some(saved_pgrp) = opt_pgrp {
            // Restore the foreground process group after resuming.
            if saved_pgrp != self.parent_pgrp {
                if let Some(tty) = opt_tty {
                    tty.tcsetpgrp_nobg(saved_pgrp).ok();
                }
            }
        }
    }

    fn on_signal(&mut self, signal: Signal, registry: &mut EventRegistry<Self>) {
        let info = match self.signal_manager.recv(signal) {
            Ok(info) => info,
            Err(err) => {
                dev_error!("sudo could not receive signal {signal:?}: {err}");
                return;
            }
        };

        dev_info!(
            "received{} {} from {}",
            opt_fmt(info.is_user_signaled(), " user signaled"),
            signal_fmt(info.signal()),
            info.pid()
        );

        let Some(command_pid) = self.command_pid else {
            dev_info!("command was terminated, ignoring signal");
            return;
        };

        match info.signal() {
            SIGCHLD => {
                self.handle_sigchld(command_pid, registry);
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExecEvent {
    Signal(Signal),
}

impl Process for ExecClosure {
    type Event = ExecEvent;
    type Break = std::convert::Infallible;
    type Exit = ExitReason;

    fn on_event(&mut self, event: Self::Event, registry: &mut EventRegistry<Self>) {
        match event {
            ExecEvent::Signal(signal) => self.on_signal(signal, registry),
        }
    }
}
