use std::collections::VecDeque;
use std::ffi::c_int;
use std::io;
use std::process::{exit, Command};

use signal_hook::consts::*;

use crate::log::{dev_error, dev_info, dev_warn};
use crate::system::signal::{SignalAction, SignalHandler};
use crate::system::term::Pty;
use crate::system::wait::{waitpid, WaitError, WaitOptions};
use crate::system::{chown, fork, Group, User};
use crate::system::{getpgid, interface::ProcessId, signal::SignalInfo};

use super::event::{EventClosure, EventDispatcher, StopReason};
use super::monitor::exec_monitor;
use super::{
    backchannel::{BackchannelPair, MonitorMessage, ParentBackchannel, ParentMessage},
    io_util::{retry_while_interrupted, was_interrupted},
    ExitReason,
};
use super::{cond_fmt, signal_fmt};

pub(super) fn exec_pty(
    sudo_pid: ProcessId,
    command: Command,
) -> io::Result<(ExitReason, impl FnOnce())> {
    // Allocate a pseudoterminal.
    // FIXME (ogsudo): We also need to open `/dev/tty`.
    let pty = get_pty()?;

    // Create backchannels to communicate with the monitor.
    let mut backchannels = BackchannelPair::new().map_err(|err| {
        dev_error!("unable to create backchannel: {err}");
        err
    })?;

    // We don't want to receive SIGTTIN/SIGTTOU
    // FIXME: why?
    if let Err(err) = SignalHandler::with_action(SIGTTIN, SignalAction::Ignore) {
        dev_error!("unable to set handler for SIGTTIN: {err}");
    }
    if let Err(err) = SignalHandler::with_action(SIGTTOU, SignalAction::Ignore) {
        dev_error!("unable to set handler for SIGTTOU: {err}");
    }

    // FIXME (ogsudo): Initialize the policy plugin's session here by calling
    // `policy_init_session`.
    // FIXME (ogsudo): initializes ttyblock sigset here by calling `init_ttyblock`
    // FIXME (ogsudo): Set all the IO streams for the command to the follower side of the pty.
    // FIXME (ogsudo): Read from `/dev/tty` and write to the leader if not in the background.
    // FIXME (ogsudo): Read from the leader and write to `/dev/tty`.
    // FIXME (ogsudo): Do some extra setup if any of the IO streams are not a tty and logging is
    // enabled or if sudo is running in background.
    // FIXME (ogsudo): Copy terminal settings from `/dev/tty` to the pty.
    // FIXME (ogsudo): Start in raw mode unless we're part of a pipeline
    // FIXME: it would be better if we didn't create the dispatcher before the fork and managed
    // to block all the signals instead.
    let mut dispatcher = EventDispatcher::<ParentClosure>::new()?;

    let monitor_pid = fork().map_err(|err| {
        dev_error!("unable to fork monitor process: {err}");
        err
    })?;

    if monitor_pid == 0 {
        // Close the file descriptors that we don't access
        drop(pty.leader);
        drop(backchannels.parent);

        // Unregister all the handlers so `exec_monitor` can register new ones for the monitor
        // process.
        dispatcher.unregister_handlers();

        // If `exec_monitor` returns, it means we failed to execute the command somehow.
        if let Err(err) = exec_monitor(pty.follower, command, &mut backchannels.monitor) {
            match err.try_into() {
                Ok(msg) => {
                    if let Err(err) = backchannels.monitor.send(&msg) {
                        dev_error!("unable to send status to parent: {err}");
                    }
                }
                Err(err) => dev_warn!("execution error {err:?} cannot be send over backchannel"),
            }
        }
        // FIXME: drop everything before calling `exit`.
        exit(1)
    }

    // Close the file descriptors that we don't access
    drop(pty.follower);
    drop(backchannels.monitor);

    // Send green light to the monitor after closing the follower.
    retry_while_interrupted(|| backchannels.parent.send(&MonitorMessage::ExecCommand)).map_err(
        |err| {
            dev_error!("unable to send green light to monitor: {err}");
            err
        },
    )?;

    let closure = ParentClosure::new(monitor_pid, sudo_pid, backchannels.parent, &mut dispatcher);

    // FIXME (ogsudo): Restore the signal handlers here.

    // FIXME (ogsudo): Retry if `/dev/tty` is revoked.
    closure
        .run(&mut dispatcher)
        .map(|exit_reason| (exit_reason, move || drop(dispatcher)))
}

fn get_pty() -> io::Result<Pty> {
    let tty_gid = Group::from_name("tty")
        .unwrap_or(None)
        .map(|group| group.gid);

    let pty = Pty::open().map_err(|err| {
        dev_error!("unable to allocate pty: {err}");
        err
    })?;
    // FIXME: Test this
    chown(&pty.path, User::effective_uid(), tty_gid).map_err(|err| {
        dev_error!("unable to change owner for pty: {err}");
        err
    })?;

    Ok(pty)
}

struct ParentClosure {
    // The monitor PID.
    //
    /// This is `Some` iff the process is still running.
    monitor_pid: Option<ProcessId>,
    sudo_pid: ProcessId,
    command_pid: Option<ProcessId>,
    backchannel: ParentBackchannel,
    message_queue: VecDeque<MonitorMessage>,
}

impl ParentClosure {
    fn new(
        monitor_pid: ProcessId,
        sudo_pid: ProcessId,
        backchannel: ParentBackchannel,
        dispatcher: &mut EventDispatcher<Self>,
    ) -> Self {
        dispatcher.set_read_callback(&backchannel, |parent, dispatcher| {
            parent.on_message_received(dispatcher)
        });

        // Check for queued messages only when the backchannel can be written so we can send
        // messages to the monitor process without blocking.
        dispatcher.set_write_callback(&backchannel, |parent, dispatcher| {
            parent.check_message_queue(dispatcher)
        });

        Self {
            monitor_pid: Some(monitor_pid),
            sudo_pid,
            command_pid: None,
            backchannel,
            message_queue: VecDeque::new(),
        }
    }

    fn run(mut self, dispatcher: &mut EventDispatcher<Self>) -> io::Result<ExitReason> {
        match dispatcher.event_loop(&mut self) {
            StopReason::Break(err) | StopReason::Exit(ParentExit::Backchannel(err)) => Err(err),
            StopReason::Exit(ParentExit::Command(exit_reason)) => Ok(exit_reason),
        }
    }

    /// Read an event from the backchannel and return the event if it should break the event loop.
    fn on_message_received(&mut self, dispatcher: &mut EventDispatcher<Self>) {
        match self.backchannel.recv() {
            // Not an actual error, we can retry later.
            Err(err) if was_interrupted(&err) => {}
            // Failed to read command status. This means that something is wrong with the socket
            // and we should stop.
            Err(err) => {
                // If we get EOF the monitor exited or was killed
                if err.kind() == io::ErrorKind::UnexpectedEof {
                    dev_info!("parent received EOF from backchannel");
                    dispatcher.set_exit(err.into());
                } else {
                    dev_error!("could not receive message from monitor: {err}");
                    if !dispatcher.got_break() {
                        dispatcher.set_break(err);
                    }
                }
            }
            Ok(event) => {
                match event {
                    // Received the PID of the command. This means that the command is already
                    // executing.
                    ParentMessage::CommandPid(pid) => {
                        dev_info!("received command PID ({pid}) from monitor");
                        self.command_pid = pid.into();
                    }
                    // The command terminated or the monitor was not able to spawn it. We should stop
                    // either way.
                    ParentMessage::CommandExit(code) => {
                        dev_info!("command exited with status code {code}");
                        dispatcher.set_exit(ExitReason::Code(code).into());
                    }
                    ParentMessage::CommandSignal(signal) => {
                        // FIXME: this isn't right as the command has not exited if the signal is
                        // not a termination one. However, doing this makes us fail an ignored
                        // compliance test instead of hanging forever.
                        dev_info!("command was terminated by {}", signal_fmt(signal));
                        dispatcher.set_exit(ExitReason::Signal(signal).into());
                    }
                    ParentMessage::IoError(code) => {
                        let err = io::Error::from_raw_os_error(code);
                        dev_info!("received error ({code}) for monitor: {err}");
                        dispatcher.set_break(err);
                    }
                    ParentMessage::ShortRead => {
                        dev_info!("received short read error for monitor");
                        dispatcher.set_break(io::ErrorKind::UnexpectedEof.into());
                    }
                }
            }
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

    /// Schedule sending a signal event to the monitor using the backchannel.
    ///
    /// The signal message will be sent once the backchannel is ready to be written.
    fn schedule_signal(&mut self, signal: c_int) {
        dev_info!("scheduling message with {} for monitor", signal_fmt(signal));
        self.message_queue.push_back(MonitorMessage::Signal(signal));
    }

    /// Send the first message in the event queue using the backchannel, if any.
    ///
    /// Calling this function will block until the backchannel can be written.
    fn check_message_queue(&mut self, dispatcher: &mut EventDispatcher<Self>) {
        if let Some(msg) = self.message_queue.front() {
            dev_info!("sending message {msg:?} to monitor over backchannel");
            match self.backchannel.send(msg) {
                // The event was sent, remove it from the queue
                Ok(()) => {
                    self.message_queue.pop_front().unwrap();
                }
                // The other end of the socket is gone, we should exit.
                Err(err) if err.kind() == io::ErrorKind::BrokenPipe => {
                    dev_error!("broken pipe while writing to monitor over backchannel");
                    // FIXME: maybe we need a different event for backchannel errors.
                    dispatcher.set_break(err);
                }
                // Non critical error, we can retry later.
                Err(_) => {}
            }
        }
    }

    /// Handle changes to the monitor status.
    fn handle_sigchld(&mut self, monitor_pid: ProcessId) {
        const OPTS: WaitOptions = WaitOptions::new().all().untraced().no_hang();

        let status = loop {
            match waitpid(monitor_pid, OPTS) {
                Err(WaitError::Io(err)) if was_interrupted(&err) => {}
                // This only happens if we receive `SIGCHLD` but there's no status update from the
                // monitor.
                Err(WaitError::Io(_err)) => dev_info!("parent could not wait for monitor: {_err}"),
                // This only happens if the monitor exited and any process already waited for the monitor.
                Err(WaitError::NotReady) => dev_info!("monitor process without status update"),
                Ok((_pid, status)) => break status,
            }
        };

        if let Some(_code) = status.exit_status() {
            dev_info!("monitor ({monitor_pid}) exited with status code {_code}");
            self.monitor_pid = None;
        } else if let Some(_signal) = status.term_signal() {
            dev_info!(
                "monitor ({monitor_pid}) was terminated by {}",
                signal_fmt(_signal)
            );
            self.monitor_pid = None;
        } else if let Some(_signal) = status.stop_signal() {
            // FIXME: we should stop too.
            dev_info!(
                "monitor ({monitor_pid}) was stopped by {}",
                signal_fmt(_signal)
            );
        } else if status.did_continue() {
            dev_info!("monitor ({monitor_pid}) continued execution");
        } else {
            dev_warn!("unexpected wait status for monitor ({monitor_pid})")
        }
    }
}

enum ParentExit {
    /// Error while reading from the backchannel.
    Backchannel(io::Error),
    /// The command exited.
    Command(ExitReason),
}

impl From<io::Error> for ParentExit {
    fn from(err: io::Error) -> Self {
        Self::Backchannel(err)
    }
}

impl From<ExitReason> for ParentExit {
    fn from(reason: ExitReason) -> Self {
        Self::Command(reason)
    }
}

impl EventClosure for ParentClosure {
    type Break = io::Error;
    type Exit = ParentExit;

    fn on_signal(&mut self, info: SignalInfo, _dispatcher: &mut EventDispatcher<Self>) {
        dev_info!(
            "parent received{} {} from {}",
            cond_fmt(" user signaled", info.is_user_signaled()),
            signal_fmt(info.signal()),
            info.pid()
        );

        let Some(monitor_pid) = self.monitor_pid else {
            dev_info!("monitor was terminated, ignoring signal");
            return;
        };

        match info.signal() {
            SIGCHLD => self.handle_sigchld(monitor_pid),
            // FIXME: check `resume_terminal`
            SIGCONT => {}
            // FIXME: check `sync_ttysize`
            SIGWINCH => {}
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if info.is_user_signaled() && self.is_self_terminating(info.pid()) => {}
            // FIXME: check `send_command_status`
            signal => self.schedule_signal(signal),
        }
    }
}
