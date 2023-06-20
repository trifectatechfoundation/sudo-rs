use std::{
    ffi::c_int,
    io::{self, Read, Write},
    os::{
        fd::OwnedFd,
        unix::{net::UnixStream, process::CommandExt},
    },
    process::{exit, Command},
    time::Duration,
};

use crate::system::{
    fork, getpgid,
    interface::ProcessId,
    kill, setpgid, setsid,
    signal::SignalInfo,
    term::{set_controlling_terminal, tcgetpgrp, tcsetpgrp},
    wait::{waitpid, WaitError, WaitOptions},
};
use crate::{
    exec::event::StopReason,
    log::{dev_info, dev_warn},
};

use signal_hook::consts::*;

use super::{
    backchannel::{MonitorBackchannel, MonitorMessage, ParentMessage},
    event::{EventClosure, EventDispatcher},
    io_util::{retry_while_interrupted, was_interrupted},
    ExitReason,
};
use super::{cond_fmt, signal_fmt};

// FIXME: This should return `io::Result<!>` but `!` is not stable yet.
pub(super) fn exec_monitor(
    pty_follower: OwnedFd,
    command: Command,
    backchannel: &mut MonitorBackchannel,
) -> io::Result<()> {
    let mut dispatcher = EventDispatcher::<MonitorClosure>::new()?;

    // FIXME (ogsudo): Any file descriptor not used by the monitor are closed here.

    // FIXME (ogsudo): SIGTTIN and SIGTTOU are ignored here but the docs state that it shouldn't
    // be possible to receive them in the first place. Investigate

    // Start a new terminal session with the monitor as the leader.
    setsid().map_err(|err| {
        dev_warn!("cannot start a new session: {err}");
        err
    })?;

    // Set the follower side of the pty as the controlling terminal for the session.
    set_controlling_terminal(&pty_follower).map_err(|err| {
        dev_warn!("cannot set the controlling terminal: {err}");
        err
    })?;

    // Use a pipe to get the IO error if `exec_command` fails.
    let (mut errpipe_tx, errpipe_rx) = UnixStream::pair()?;

    // Wait for the parent to give us green light before spawning the command. This avoids race
    // conditions when the command exits quickly.
    let event = retry_while_interrupted(|| backchannel.recv()).map_err(|err| {
        dev_warn!("cannot receive green light from parent: {err}");
        err
    })?;
    // Given that `UnixStream` delivers messages in order it shouldn't be possible to
    // receive an event different to `ExecCommand` at the beginning.
    debug_assert_eq!(event, MonitorMessage::ExecCommand);

    // FIXME (ogsudo): Some extra config happens here if selinux is available.

    let command_pid = fork().map_err(|err| {
        dev_warn!("unable to fork command process: {err}");
        err
    })?;

    if command_pid == 0 {
        drop(errpipe_rx);

        let err = exec_command(command, pty_follower);
        dev_warn!("failed to execute command: {err}");
        // If `exec_command` returns, it means that executing the command failed. Send the error to
        // the monitor using the pipe.
        if let Some(error_code) = err.raw_os_error() {
            errpipe_tx.write_all(&error_code.to_ne_bytes()).ok();
        }
        drop(errpipe_tx);
        // FIXME: Calling `exit` doesn't run any destructors, clean everything up.
        exit(1)
    }

    // Send the command's PID to the parent.
    if let Err(err) = backchannel.send(&ParentMessage::CommandPid(command_pid)) {
        dev_warn!("cannot send command PID to parent: {err}");
    }

    let mut closure = MonitorClosure::new(command_pid, errpipe_rx, backchannel, &mut dispatcher);

    // Set the foreground group for the pty follower.
    tcsetpgrp(&pty_follower, closure.command_pgrp).ok();

    // FIXME (ogsudo): Here's where the signal mask is removed because the handlers for the signals
    // have been setup after initializing the closure.
    // FIXME (ogsudo): Set the command as the foreground process for the follower.

    // Start the event loop.
    let reason = dispatcher.event_loop(&mut closure);
    // FIXME (ogsudo): Terminate the command using `killpg` if it's not terminated.
    // FIXME (ogsudo): Take the controlling tty so the command's children don't receive SIGHUP when we exit.

    match reason {
        StopReason::Break(()) => {}
        StopReason::Exit(exit_reason) => {
            closure.backchannel.send(&exit_reason.into()).ok();
        }
    }

    // FIXME (ogsudo): The tty is restored here if selinux is available.

    drop(closure);

    exit(1)
}

// FIXME: This should return `io::Result<!>` but `!` is not stable yet.
fn exec_command(mut command: Command, pty_follower: OwnedFd) -> io::Error {
    // FIXME (ogsudo): Do any additional configuration that needs to be run after `fork` but before `exec`
    let command_pid = std::process::id() as ProcessId;

    setpgid(0, command_pid).ok();

    // Wait for the monitor to set us as the foreground group for the pty.
    while !tcgetpgrp(&pty_follower).is_ok_and(|pid| pid == command_pid) {
        std::thread::sleep(std::time::Duration::from_micros(1));
    }

    command.exec()
}

struct MonitorClosure<'a> {
    /// The command PID.
    ///
    /// This is `Some` iff the process is still running.
    command_pid: Option<ProcessId>,
    command_pgrp: ProcessId,
    errpipe_rx: UnixStream,
    backchannel: &'a mut MonitorBackchannel,
}

impl<'a> MonitorClosure<'a> {
    fn new(
        command_pid: ProcessId,
        errpipe_rx: UnixStream,
        backchannel: &'a mut MonitorBackchannel,
        dispatcher: &mut EventDispatcher<Self>,
    ) -> Self {
        // FIXME (ogsudo): Store the pgid of the monitor.

        // Register the callback to receive the IO error if the command fails to execute.
        dispatcher.set_read_callback(&errpipe_rx, |monitor, dispatcher| {
            monitor.read_errpipe(dispatcher)
        });

        // Register the callback to receive events from the backchannel
        dispatcher.set_read_callback(backchannel, |monitor, dispatcher| {
            monitor.read_backchannel(dispatcher)
        });

        // Put the command in its own process group.
        let command_pgrp = command_pid;
        if let Err(err) = setpgid(command_pid, command_pgrp) {
            dev_warn!("cannot set process group ID for process: {err}");
        };

        Self {
            command_pid: Some(command_pid),
            command_pgrp,
            errpipe_rx,
            backchannel,
        }
    }

    /// Based on `mon_backchannel_cb`
    fn read_backchannel(&mut self, dispatcher: &mut EventDispatcher<Self>) {
        match self.backchannel.recv() {
            // Read interrupted, we can try again later.
            Err(err) if was_interrupted(&err) => {}
            // There's something wrong with the backchannel, break the event loop
            Err(err) => {
                dev_warn!("monitor could not read from backchannel: {}", err);
                // FIXME: maybe the break reason should be `io::Error` instead.
                dispatcher.set_break(());
                match err.try_into() {
                    Ok(msg) => {
                        if let Err(err) = self.backchannel.send(&msg) {
                            dev_warn!("monitor could not write to the backchannel: {err}");
                        }
                    }
                    Err(err) => {
                        dev_warn!("backchannel read error {err:?} cannot be send over backchannel")
                    }
                }
            }
            Ok(event) => {
                match event {
                    // We shouldn't receive this event more than once.
                    MonitorMessage::ExecCommand => unreachable!(),
                    // Forward signal to the command.
                    MonitorMessage::Signal(signal) => {
                        if let Some(command_pid) = self.command_pid {
                            send_signal(signal, command_pid, true)
                        }
                    }
                }
            }
        }
    }

    fn handle_sigchld(&mut self, command_pid: ProcessId, dispatcher: &mut EventDispatcher<Self>) {
        let status = loop {
            match waitpid(command_pid, WaitOptions::new().untraced().no_hang()) {
                Ok((_pid, status)) => break status,
                Err(WaitError::Io(err)) if was_interrupted(&err) => {}
                Err(_) => return,
            }
        };

        if let Some(exit_code) = status.exit_status() {
            dev_info!("command ({command_pid}) exited with status code {exit_code}");
            // The command did exit, set it's PID to `None`.
            self.command_pid = None;
            dispatcher.set_exit(ExitReason::Code(exit_code))
        } else if let Some(signal) = status.term_signal() {
            dev_info!(
                "command ({command_pid}) was terminated by {}",
                signal_fmt(signal),
            );
            // The command was terminated, set it's PID to `None`.
            self.command_pid = None;
            dispatcher.set_exit(ExitReason::Signal(signal))
        } else if let Some(signal) = status.stop_signal() {
            dev_info!(
                "command ({command_pid}) was stopped by {}",
                signal_fmt(signal),
            );
            // FIXME: we should save the foreground process group ID so we can restore it later.
            self.backchannel
                .send(&ParentMessage::CommandSignal(signal))
                .ok();
        } else if status.did_continue() {
            dev_info!("command ({command_pid}) continued execution");
        } else {
            dev_warn!("unexpected wait status for command ({command_pid})")
        }
    }

    fn read_errpipe(&mut self, dispatcher: &mut EventDispatcher<Self>) {
        let mut buf = 0i32.to_ne_bytes();
        match self.errpipe_rx.read_exact(&mut buf) {
            Err(err) if was_interrupted(&err) => { /* Retry later */ }
            Err(err) => {
                // Could not read from the pipe, report error to the parent.
                // FIXME: Maybe we should have a different variant for this.
                match err.try_into() {
                    Ok(msg) => {
                        self.backchannel.send(&msg).ok();
                    }
                    Err(err) => {
                        dev_warn!("pipe read error {err:?} cannot be send over backchannel")
                    }
                }

                dispatcher.set_break(());
            }
            Ok(_) => {
                // Received error code from the command, forward it to the parent.
                let error_code = i32::from_ne_bytes(buf);
                self.backchannel
                    .send(&ParentMessage::IoError(error_code))
                    .ok();
            }
        }
    }
}

/// Send a signal to the command.
fn send_signal(signal: c_int, command_pid: ProcessId, from_parent: bool) {
    dev_info!(
        "sending {}{} to command",
        signal_fmt(signal),
        cond_fmt(" from parent", from_parent),
    );
    // FIXME: We should call `killpg` instead of `kill`.
    match signal {
        SIGALRM => {
            // Kill the command with increasing urgency. Based on `terminate_command`.
            kill(command_pid, SIGHUP).ok();
            kill(command_pid, SIGTERM).ok();
            std::thread::sleep(Duration::from_secs(2));
            kill(command_pid, SIGKILL).ok();
        }
        signal => {
            // Send the signal to the command.
            kill(command_pid, signal).ok();
        }
    }
}

/// Decides if the signal sent by the process with `signaler_pid` PID is self-terminating.
///
/// A signal is self-terminating if `signaler_pid`:
/// - is the same PID of the command, or
/// - is in the process group of the command and the command is the leader.
fn is_self_terminating(
    signaler_pid: ProcessId,
    command_pid: ProcessId,
    command_pgrp: ProcessId,
) -> bool {
    if signaler_pid != 0 {
        if signaler_pid == command_pid {
            return true;
        }

        if let Ok(grp_leader) = getpgid(signaler_pid) {
            if grp_leader == command_pgrp {
                return true;
            }
        }
    }

    false
}

impl<'a> EventClosure for MonitorClosure<'a> {
    type Break = ();
    type Exit = ExitReason;

    fn on_signal(&mut self, info: SignalInfo, dispatcher: &mut EventDispatcher<Self>) {
        dev_info!(
            "monitor received{} {} from {}",
            cond_fmt(" user signaled", info.is_user_signaled()),
            signal_fmt(info.signal()),
            info.pid()
        );

        // Don't do anything if the command has terminated already
        let Some(command_pid) = self.command_pid else {
            dev_info!("command was terminated, ignoring signal");
            return;
        };

        match info.signal() {
            SIGCHLD => self.handle_sigchld(command_pid, dispatcher),
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if info.is_user_signaled()
                && is_self_terminating(info.pid(), command_pid, self.command_pgrp) => {}
            signal => send_signal(signal, command_pid, false),
        }
    }
}
