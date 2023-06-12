use std::{
    ffi::c_int,
    io,
    os::fd::OwnedFd,
    process::{exit, Child, Command},
    time::Duration,
};

use signal_hook::consts::*;
use sudo_log::user_error;
use sudo_system::{
    getpgid, interface::ProcessId, kill, setpgid, setsid, signal::SignalInfo,
    term::set_controlling_terminal,
};

use crate::{
    backchannel::{MonitorBackchannel, MonitorMessage, ParentMessage},
    event::{EventClosure, EventDispatcher},
    io_util::{retry_while_interrupted, was_interrupted},
};

pub(super) struct MonitorClosure {
    /// The command PID.
    ///
    /// This is `Some` iff the process is still running.
    command_pid: Option<ProcessId>,
    command_pgrp: ProcessId,
    command: Child,
    _pty_follower: OwnedFd,
    backchannel: MonitorBackchannel,
}

impl MonitorClosure {
    pub(super) fn new(
        mut command: Command,
        pty_follower: OwnedFd,
        mut backchannel: MonitorBackchannel,
    ) -> (Self, EventDispatcher<Self>) {
        let result = io::Result::Ok(()).and_then(|()| {
            let mut dispatcher = EventDispatcher::<Self>::new()?;

            // Create new terminal session.
            setsid()?;

            // Set the pty as the controlling terminal.
            set_controlling_terminal(&pty_follower)?;

            // Wait for the main sudo process to give us green light before spawning the command. This
            // avoids race conditions when the command exits quickly.
            let event = retry_while_interrupted(|| backchannel.recv())?;

            // Given that `UnixStream` delivers messages in order it shouldn't be possible to
            // receive an event different to `ExecCommand` at the beginning.
            debug_assert_eq!(event, MonitorMessage::ExecCommand);

            // spawn the command
            let command = command.spawn()?;

            let command_pid = command.id() as ProcessId;

            // Send the command's PID to the main sudo process.
            backchannel
                .send(&ParentMessage::CommandPid(command_pid))
                .ok();

            // Register the callback to receive events from the backchannel
            dispatcher.set_read_callback(&backchannel, |mc, ev| mc.read_backchannel(ev));

            // set the process group ID of the command to the command PID.
            let command_pgrp = command_pid;
            setpgid(command_pid, command_pgrp).ok();

            Ok((dispatcher, command_pid, command_pgrp, command, pty_follower))
        });

        match result {
            Err(err) => {
                backchannel.send(&err.into()).unwrap();
                exit(1);
            }
            Ok((dispatcher, command_pid, command_pgrp, command, pty_follower)) => (
                Self {
                    command_pid: Some(command_pid),
                    command_pgrp,
                    command,
                    _pty_follower: pty_follower,
                    backchannel,
                },
                dispatcher,
            ),
        }
    }

    pub(super) fn run(mut self, dispatcher: &mut EventDispatcher<Self>) -> ! {
        dispatcher.event_loop(&mut self);
        drop(self);
        exit(0);
    }

    /// Based on `mon_backchannel_cb`
    fn read_backchannel(&mut self, dispatcher: &mut EventDispatcher<Self>) {
        match self.backchannel.recv() {
            // Read interrupted, we can try again later.
            Err(err) if was_interrupted(&err) => {}
            // There's something wrong with the backchannel, break the event loop
            Err(err) => {
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
                            Self::send_signal(signal, command_pid)
                        }
                    }
                }
            }
        }
    }

    /// Send a signal to the command
    fn send_signal(signal: c_int, command_pid: ProcessId) {
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
        } else {
            user_error!("Could not fetch process group ID");
        }
    }

    false
}

impl EventClosure for MonitorClosure {
    type Break = ();

    fn on_signal(&mut self, info: SignalInfo, dispatcher: &mut EventDispatcher<Self>) {
        // Don't do anything if the command has terminated already
        let Some(command_pid) = self.command_pid else {
            return;
        };

        match info.signal() {
            // FIXME: check `mon_handle_sigchld`
            SIGCHLD => {
                if let Ok(Some(exit_status)) = self.command.try_wait() {
                    // The command has terminated, set it's PID to `None`.
                    self.command_pid = None;
                    dispatcher.set_break(());
                    self.backchannel.send(&exit_status.into()).unwrap();
                }
            }
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if info.is_user_signaled()
                && is_self_terminating(info.pid(), command_pid, self.command_pgrp) => {}
            signal => Self::send_signal(signal, command_pid),
        }
    }
}
