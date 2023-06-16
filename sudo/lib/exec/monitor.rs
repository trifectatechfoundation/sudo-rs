use std::{
    ffi::c_int,
    io,
    os::fd::OwnedFd,
    process::{exit, Child, Command},
    time::Duration,
};

use crate::log::{dev_info, dev_warn};
use crate::system::{
    getpgid, interface::ProcessId, kill, setpgid, setsid, signal::SignalInfo,
    term::set_controlling_terminal,
};

use signal_hook::consts::*;

use super::{
    backchannel::{MonitorBackchannel, MonitorMessage, ParentMessage},
    event::{EventClosure, EventDispatcher},
    io_util::{retry_while_interrupted, was_interrupted},
};
use super::{cond_fmt, signal_fmt};

// FIXME: This should return `io::Result<!>` but `!` is not stable yet.
pub(super) fn exec_monitor(
    pty_follower: OwnedFd,
    mut command: Command,
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

    // FIXME (ogsudo): Do any additional configuration that needs to be run after `fork` but before `exec`.

    // spawn the command.
    let command = command.spawn().map_err(|err| {
        dev_warn!("cannot spawn command: {err}");
        err
    })?;

    let command_pid = command.id() as ProcessId;

    // Send the command's PID to the parent.
    if let Err(err) = backchannel.send(&ParentMessage::CommandPid(command_pid)) {
        dev_warn!("cannot send command PID to parent: {err}");
    }

    let mut closure = MonitorClosure::new(command, command_pid, backchannel, &mut dispatcher);

    // FIXME (ogsudo): Here's where the signal mask is removed because the handlers for the signals
    // have been setup after initializing the closure.
    // FIXME (ogsudo): Set the command as the foreground process for the follower.

    // Start the event loop.
    dispatcher.event_loop(&mut closure);
    // FIXME (ogsudo): Terminate the command using `killpg` if it's not terminated.
    // FIXME (ogsudo): Take the controlling tty so the command's children don't receive SIGHUP when we exit.
    // FIXME (ogsudo): Send the command status back to the parent.
    // FIXME (ogsudo): The tty is restored here if selinux is available.

    drop(closure);

    exit(1)
}

struct MonitorClosure<'a> {
    command: Child,
    /// The command PID.
    ///
    /// This is `Some` iff the process is still running.
    command_pid: Option<ProcessId>,
    command_pgrp: ProcessId,
    backchannel: &'a mut MonitorBackchannel,
}

impl<'a> MonitorClosure<'a> {
    fn new(
        command: Child,
        command_pid: ProcessId,
        backchannel: &'a mut MonitorBackchannel,
        dispatcher: &mut EventDispatcher<Self>,
    ) -> Self {
        // FIXME (ogsudo): Store the pgid of the monitor.

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
            command,
            command_pid: Some(command_pid),
            command_pgrp,
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
                dispatcher.set_break(());
                self.backchannel.send(&err.into()).unwrap();
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
            // FIXME: check `mon_handle_sigchld`
            SIGCHLD => {
                if let Ok(Some(exit_status)) = self.command.try_wait() {
                    dev_info!(
                        "command ({command_pid}) exited with status: {}",
                        exit_status
                    );
                    // The command has terminated, set it's PID to `None`.
                    self.command_pid = None;
                    dispatcher.set_break(());
                    self.backchannel.send(&exit_status.into()).unwrap();
                }
            }
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if info.is_user_signaled()
                && is_self_terminating(info.pid(), command_pid, self.command_pgrp) => {}
            signal => send_signal(signal, command_pid, false),
        }
    }
}
