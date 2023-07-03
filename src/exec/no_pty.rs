use std::{
    ffi::c_int,
    io::{self, Read, Write},
    os::unix::{net::UnixStream, process::CommandExt},
    process::{exit, Command},
};

use signal_hook::consts::*;

use super::{
    event::{EventRegistry, Process, StopReason},
    io_util::was_interrupted,
    terminate_process, ExitReason, HandleSigchld,
};
use crate::{
    exec::{handle_sigchld, opt_fmt, signal_fmt},
    log::{dev_error, dev_info, dev_warn},
    system::{
        fork, getpgid, getpgrp,
        interface::ProcessId,
        kill, killpg,
        poll::PollEvent,
        signal::{Signal, SignalAction, SignalHandler, SignalNumber},
        term::{Terminal, UserTerm},
        wait::WaitOptions,
        FileCloser, ForkResult,
    },
};

pub(crate) fn exec_no_pty(
    sudo_pid: ProcessId,
    mut command: Command,
) -> io::Result<(ExitReason, Box<dyn FnOnce()>)> {
    // FIXME (ogsudo): Initialize the policy plugin's session here.

    // FIXME: block signals directly instead of using the manager.
    let signal_handler = SignalHandler::new()?;

    let mut file_closer = FileCloser::new();

    // FIXME (ogsudo): Some extra config happens here if selinux is available.

    // Use a pipe to get the IO error if `exec` fails.
    let (mut errpipe_tx, errpipe_rx) = UnixStream::pair()?;

    // Don't close the error pipe as we need it to retrieve the error code if the command execution
    // fails.
    file_closer.except(&errpipe_tx);

    let ForkResult::Parent(command_pid) = fork().map_err(|err| {
        dev_warn!("unable to fork command process: {err}");
        err
    })?
    else {
        file_closer.close_the_universe()?;

        let err = command.exec();

        dev_warn!("failed to execute command: {err}");
        // If `exec` returns, it means that executing the command failed. Send the error to the
        // monitor using the pipe.
        if let Some(error_code) = err.raw_os_error() {
            errpipe_tx.write_all(&error_code.to_ne_bytes()).ok();
        }
        drop(errpipe_tx);
        // FIXME: Calling `exit` doesn't run any destructors, clean everything up.
        exit(1)
    };

    dev_info!("executed command with pid {command_pid}");

    let mut registry = EventRegistry::new();

    let mut closure = ExecClosure::new(
        command_pid,
        sudo_pid,
        errpipe_rx,
        signal_handler,
        &mut registry,
    );

    // FIXME: restore signal mask here.

    let exit_reason = match registry.event_loop(&mut closure) {
        StopReason::Break(err) => return Err(err),
        StopReason::Exit(reason) => reason,
    };

    Ok((exit_reason, Box::new(move || drop(registry))))
}

struct ExecClosure {
    command_pid: Option<ProcessId>,
    sudo_pid: ProcessId,
    parent_pgrp: ProcessId,
    errpipe_rx: UnixStream,
    signal_handler: SignalHandler,
}

impl ExecClosure {
    fn new(
        command_pid: ProcessId,
        sudo_pid: ProcessId,
        errpipe_rx: UnixStream,
        signal_handler: SignalHandler,
        registry: &mut EventRegistry<Self>,
    ) -> Self {
        registry.register_event(&signal_handler, PollEvent::Readable, |_| ExecEvent::Signal);
        registry.register_event(&errpipe_rx, PollEvent::Readable, |_| ExecEvent::ErrPipe);

        Self {
            command_pid: Some(command_pid),
            errpipe_rx,
            sudo_pid,
            parent_pgrp: getpgrp(),
            signal_handler,
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
            self.signal_handler
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
            self.signal_handler.set_action(Signal::SIGTSTP, action);
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

    fn on_signal(&mut self, registry: &mut EventRegistry<Self>) {
        let info = match self.signal_handler.recv() {
            Ok(info) => info,
            Err(err) => {
                dev_error!("sudo could not receive signal: {err}");
                return;
            }
        };

        dev_info!(
            "received{} {} from {}",
            opt_fmt(info.is_user_signaled(), " user signaled"),
            info.signal(),
            info.pid()
        );

        let Some(command_pid) = self.command_pid else {
            dev_info!("command was terminated, ignoring signal");
            return;
        };

        match info.signal() {
            Signal::SIGCHLD => handle_sigchld(self, registry, "command", command_pid),
            signal => {
                if signal == Signal::SIGWINCH {
                    // FIXME: check `handle_sigwinch_no_pty`.
                }
                // Skip the signal if it was sent by the user and it is self-terminating.
                if info.is_user_signaled() && self.is_self_terminating(info.pid()) {
                    return;
                }

                if signal == Signal::SIGALRM {
                    terminate_process(command_pid, false);
                } else {
                    kill(command_pid, signal.number()).ok();
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecEvent {
    Signal,
    ErrPipe,
}

impl Process for ExecClosure {
    type Event = ExecEvent;
    type Break = io::Error;
    type Exit = ExitReason;

    fn on_event(&mut self, event: Self::Event, registry: &mut EventRegistry<Self>) {
        match event {
            ExecEvent::Signal => self.on_signal(registry),
            ExecEvent::ErrPipe => {
                let mut buf = 0i32.to_ne_bytes();
                match self.errpipe_rx.read_exact(&mut buf) {
                    Err(err) if was_interrupted(&err) => { /* Retry later */ }
                    Err(err) => registry.set_break(err),
                    Ok(_) => {
                        // Received error code from the command, forward it to the parent.
                        let error_code = i32::from_ne_bytes(buf);
                        registry.set_break(io::Error::from_raw_os_error(error_code));
                    }
                }
            }
        }
    }
}

impl HandleSigchld for ExecClosure {
    const OPTIONS: WaitOptions = WaitOptions::new().all().untraced().no_hang();

    fn on_exit(&mut self, exit_code: c_int, registry: &mut EventRegistry<Self>) {
        registry.set_exit(ExitReason::Code(exit_code));
        self.command_pid = None;
    }

    fn on_term(&mut self, signal: SignalNumber, registry: &mut EventRegistry<Self>) {
        registry.set_exit(ExitReason::Signal(signal));
        self.command_pid = None;
    }

    fn on_stop(&mut self, signal: SignalNumber, _registry: &mut EventRegistry<Self>) {
        self.suspend_parent(signal);
    }
}
