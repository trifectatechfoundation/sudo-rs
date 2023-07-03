use std::collections::VecDeque;
use std::ffi::c_int;
use std::io::{self, IsTerminal};
use std::process::{exit, Command, Stdio};

use signal_hook::consts::*;

use crate::exec::event::{EventHandle, EventRegistry, Process, StopReason};
use crate::exec::use_pty::monitor::exec_monitor;
use crate::exec::use_pty::SIGCONT_FG;
use crate::exec::{
    cond_fmt, handle_sigchld, opt_fmt, signal_fmt, terminate_process, HandleSigchld,
};
use crate::exec::{
    io_util::{retry_while_interrupted, was_interrupted},
    use_pty::backchannel::{BackchannelPair, MonitorMessage, ParentBackchannel, ParentMessage},
    ExitReason,
};
use crate::log::{dev_error, dev_info, dev_warn};
use crate::system::poll::PollEvent;
use crate::system::signal::{Signal, SignalHandler};
use crate::system::signal::{SignalAction, SignalNumber};
use crate::system::term::{Pty, PtyFollower, PtyLeader, Terminal, UserTerm};
use crate::system::wait::WaitOptions;
use crate::system::{chown, fork, getpgrp, kill, killpg, FileCloser, ForkResult, Group, User};
use crate::system::{getpgid, interface::ProcessId};

use super::pipe::Pipe;
use super::{CommandStatus, SIGCONT_BG};

pub(crate) fn exec_pty(
    sudo_pid: ProcessId,
    mut command: Command,
    user_tty: UserTerm,
) -> io::Result<(ExitReason, Box<dyn FnOnce()>)> {
    // Allocate a pseudoterminal.
    let pty = get_pty()?;

    // Create backchannels to communicate with the monitor.
    let mut backchannels = BackchannelPair::new().map_err(|err| {
        dev_error!("cannot create backchannel: {err}");
        err
    })?;

    // We don't want to receive SIGTTIN/SIGTTOU
    // FIXME: why?
    let signal_handler = SignalHandler::with_actions(|signal| match signal {
        Signal::SIGTTIN | Signal::SIGTTOU => SignalAction::Ignore,
        _ => SignalAction::Default,
    })?;

    // FIXME (ogsudo): Initialize the policy plugin's session here by calling
    // `policy_init_session`.
    // FIXME (ogsudo): initializes ttyblock sigset here by calling `init_ttyblock`

    // Fetch the parent process group so we can signals to it.
    let parent_pgrp = getpgrp();

    let mut file_closer = FileCloser::new();

    // Set all the IO streams for the command to the follower side of the pty.
    let mut clone_follower = || -> io::Result<PtyFollower> {
        let follower = pty.follower.try_clone().map_err(|err| {
            dev_error!("cannot clone pty follower: {err}");
            err
        })?;
        // Don't close these as we will need them so they are dupped inside `Command::exec`.
        file_closer.except(&follower);

        Ok(follower)
    };

    command.stdin(clone_follower()?);
    command.stdout(clone_follower()?);
    command.stderr(clone_follower()?);

    let mut registry = EventRegistry::<ParentClosure>::new();

    // Pipe data between both terminals
    let mut tty_pipe = Pipe::new(
        user_tty,
        pty.leader,
        &mut registry,
        ParentEvent::Tty,
        ParentEvent::Pty,
    );

    let user_tty = tty_pipe.left_mut();

    // Check if we are the foreground process
    let mut foreground = user_tty
        .tcgetpgrp()
        .is_ok_and(|tty_pgrp| tty_pgrp == parent_pgrp);
    dev_info!(
        "sudo is runnning in the {}",
        cond_fmt(foreground, "foreground", "background")
    );

    // FIXME: maybe all these boolean flags should be on a dedicated type.

    // Whether we're running on a pipeline
    let mut pipeline = false;
    // Whether the command should be executed in the background (this is not the `-b` flag)
    let mut exec_bg = false;
    // Whether the user's terminal is in raw mode or not.
    let mut term_raw = false;

    // Check if we are part of a pipeline.
    // FIXME: Here's where we should intercept the IO streams if we want to implement IO logging.
    // FIXME: ogsudo creates pipes for the IO streams and uses events to read from the strams to
    // the pipes. Investigate why.
    if !io::stdin().is_terminal() {
        dev_info!("stdin is not a terminal, command will inherit it");
        pipeline = true;
        command.stdin(Stdio::inherit());

        if foreground && parent_pgrp != sudo_pid {
            // If sudo is not the process group leader and stdin is not a terminal we might be
            // running as a background job via a shell script. Starting in the foreground would
            // change the terminal mode.
            exec_bg = true;
        }
    }

    if !io::stdout().is_terminal() {
        dev_info!("stdout is not a terminal, command will inherit it");
        pipeline = true;
        command.stdout(Stdio::inherit());
    }

    if !io::stderr().is_terminal() {
        dev_info!("stderr is not a terminal, command will inherit it");
        command.stderr(Stdio::inherit());
    }

    // Copy terminal settings from `/dev/tty` to the pty.
    if let Err(err) = user_tty.copy_to(&pty.follower) {
        dev_error!("cannot copy terminal settings to pty: {err}");
        foreground = false;
    }

    // Start in raw mode unless we're part of a pipeline or backgrounded.
    if foreground && !pipeline && !exec_bg && user_tty.set_raw_mode(false).is_ok() {
        term_raw = true;
    }

    // FIXME: it would be better if we didn't create the manager before the fork and managed
    // to block all the signals here instead.
    for &signal in Signal::ALL {
        if signal != Signal::SIGTTIN && signal != Signal::SIGTTOU {
            signal_handler.set_action(signal, SignalAction::Stream);
        }
    }

    let ForkResult::Parent(monitor_pid) = fork().map_err(|err| {
        dev_error!("cannot fork monitor process: {err}");
        err
    })?
    else {
        // Close the file descriptors that we don't access
        drop(tty_pipe);
        drop(backchannels.parent);

        // Unregister all the handlers so `exec_monitor` can register new ones for the monitor
        // process.
        signal_handler.unregister();

        // If `exec_monitor` returns, it means we failed to execute the command somehow.
        if let Err(err) = exec_monitor(
            pty.follower,
            command,
            foreground && !pipeline && !exec_bg,
            &mut backchannels.monitor,
            file_closer,
        ) {
            match err.try_into() {
                Ok(msg) => {
                    if let Err(err) = backchannels.monitor.send(&msg) {
                        dev_error!("cannot send status to parent: {err}");
                    }
                }
                Err(err) => dev_warn!("execution error {err:?} cannot be send over backchannel"),
            }
        }
        // FIXME: drop everything before calling `exit`.
        exit(1)
    };

    // Close the file descriptors that we don't access
    drop(pty.follower);
    drop(backchannels.monitor);

    // Send green light to the monitor after closing the follower.
    retry_while_interrupted(|| backchannels.parent.send(&MonitorMessage::ExecCommand)).map_err(
        |err| {
            dev_error!("cannot send green light to monitor: {err}");
            err
        },
    )?;

    let mut closure = ParentClosure::new(
        monitor_pid,
        sudo_pid,
        parent_pgrp,
        backchannels.parent,
        tty_pipe,
        foreground,
        term_raw,
        signal_handler,
        &mut registry,
    );

    // FIXME (ogsudo): Restore the signal handlers here.

    let exit_reason = closure.run(&mut registry);
    // FIXME (ogsudo): Retry if `/dev/tty` is revoked.

    // Flush the terminal
    closure.tty_pipe.flush_left().ok();

    // Restore the terminal settings
    if closure.term_raw {
        // Only restore the terminal if sudo is the foreground process.
        if let Ok(pgrp) = closure.tty_pipe.left().tcgetpgrp() {
            if pgrp == closure.parent_pgrp {
                match closure.tty_pipe.left_mut().restore(false) {
                    Ok(()) => closure.term_raw = false,
                    Err(err) => dev_warn!("cannot restore terminal settings: {err}"),
                }
            }
        }
    }

    match exit_reason {
        Ok(exit_reason) => Ok((exit_reason, Box::new(move || drop(registry)))),
        Err(err) => Err(err),
    }
}

fn get_pty() -> io::Result<Pty> {
    let tty_gid = Group::from_name("tty")
        .unwrap_or(None)
        .map(|group| group.gid);

    let pty = Pty::open().map_err(|err| {
        dev_error!("cannot allocate pty: {err}");
        err
    })?;

    chown(&pty.path, User::effective_uid(), tty_gid).map_err(|err| {
        dev_error!("cannot change owner for pty: {err}");
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
    parent_pgrp: ProcessId,
    command_pid: Option<ProcessId>,
    tty_pipe: Pipe<UserTerm, PtyLeader>,
    foreground: bool,
    term_raw: bool,
    backchannel: ParentBackchannel,
    message_queue: VecDeque<MonitorMessage>,
    backchannel_write_handle: EventHandle,
    signal_handler: SignalHandler,
}

impl ParentClosure {
    #[allow(clippy::too_many_arguments)]
    fn new(
        monitor_pid: ProcessId,
        sudo_pid: ProcessId,
        parent_pgrp: ProcessId,
        backchannel: ParentBackchannel,
        tty_pipe: Pipe<UserTerm, PtyLeader>,
        foreground: bool,
        term_raw: bool,
        signal_handler: SignalHandler,
        registry: &mut EventRegistry<Self>,
    ) -> Self {
        registry.register_event(&backchannel, PollEvent::Readable, ParentEvent::Backchannel);
        let mut backchannel_write_handle =
            registry.register_event(&backchannel, PollEvent::Writable, ParentEvent::Backchannel);
        // Ignore write events on the backchannel as we don't want to poll it for writing if there
        // are no messages in the queue.
        backchannel_write_handle.ignore(registry);

        registry.register_event(&signal_handler, PollEvent::Readable, |_| {
            ParentEvent::Signal
        });

        Self {
            monitor_pid: Some(monitor_pid),
            sudo_pid,
            parent_pgrp,
            command_pid: None,
            tty_pipe,
            foreground,
            term_raw,
            backchannel,
            message_queue: VecDeque::new(),
            backchannel_write_handle,
            signal_handler,
        }
    }

    fn run(&mut self, registry: &mut EventRegistry<Self>) -> io::Result<ExitReason> {
        match registry.event_loop(self) {
            StopReason::Break(err) | StopReason::Exit(ParentExit::Backchannel(err)) => Err(err),
            StopReason::Exit(ParentExit::Command(exit_reason)) => Ok(exit_reason),
        }
    }

    /// Read an event from the backchannel and return the event if it should break the event loop.
    fn on_message_received(&mut self, registry: &mut EventRegistry<Self>) {
        match self.backchannel.recv() {
            // Not an actual error, we can retry later.
            Err(err) if was_interrupted(&err) => {}
            // Failed to read command status. This means that something is wrong with the socket
            // and we should stop.
            Err(err) => {
                // If we get EOF the monitor exited or was killed
                if err.kind() == io::ErrorKind::UnexpectedEof {
                    dev_info!("received EOF from backchannel");
                    registry.set_exit(err.into());
                } else {
                    dev_error!("cannot receive message from backchannel: {err}");
                    if !registry.got_break() {
                        registry.set_break(err);
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
                    ParentMessage::CommandStatus(status) => {
                        // The command terminated or the monitor was not able to spawn it. We should stop
                        // either way.
                        match status {
                            CommandStatus::Exit(exit_code) => {
                                dev_info!("command exited with status code {exit_code}");
                                registry.set_exit(ExitReason::Code(exit_code).into());
                            }
                            CommandStatus::Term(signal) => {
                                dev_info!("command was terminated by {}", signal_fmt(signal));
                                registry.set_exit(ExitReason::Signal(signal).into());
                            }
                            CommandStatus::Stop(signal) => {
                                dev_info!(
                                    "command was stopped by {}, suspending parent",
                                    signal_fmt(signal)
                                );
                                // Suspend parent and tell monitor how to resume on return
                                if let Some(signal) = self.suspend_pty(signal, registry) {
                                    self.schedule_signal(signal, registry);
                                }

                                self.tty_pipe.resume_events(registry);
                            }
                        }
                    }
                    ParentMessage::IoError(code) => {
                        let err = io::Error::from_raw_os_error(code);
                        dev_info!("received error ({code}) for monitor: {err}");
                        registry.set_break(err);
                    }
                    ParentMessage::ShortRead => {
                        dev_info!("received short read error for monitor");
                        registry.set_break(io::ErrorKind::UnexpectedEof.into());
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
    fn schedule_signal(&mut self, signal: c_int, registry: &mut EventRegistry<Self>) {
        dev_info!("scheduling message with {} for monitor", signal_fmt(signal));
        self.message_queue.push_back(MonitorMessage::Signal(signal));

        // Start polling the backchannel for writing if not already.
        self.backchannel_write_handle.resume(registry);
    }

    /// Send the first message in the event queue using the backchannel, if any.
    ///
    /// Calling this function will block until the backchannel can be written.
    fn check_message_queue(&mut self, registry: &mut EventRegistry<Self>) {
        if let Some(msg) = self.message_queue.front() {
            dev_info!("sending message {msg:?} to monitor over backchannel");
            match self.backchannel.send(msg) {
                // The event was sent, remove it from the queue
                Ok(()) => {
                    self.message_queue.pop_front().unwrap();
                    // Stop polling the backchannel for writing if the queue is empty.
                    if self.message_queue.is_empty() {
                        self.backchannel_write_handle.ignore(registry);
                    }
                }
                // The other end of the socket is gone, we should exit.
                Err(err) if err.kind() == io::ErrorKind::BrokenPipe => {
                    dev_error!("broken pipe while writing to monitor over backchannel");
                    // FIXME: maybe we need a different event for backchannel errors.
                    registry.set_break(err);
                }
                // Non critical error, we can retry later.
                Err(_) => {}
            }
        }
    }

    /// Suspend sudo if the command is suspended.
    ///
    /// Return `SIGCONT_FG` or `SIGCONT_BG` to state whether the command should be resumend in the
    /// foreground or not.
    fn suspend_pty(
        &mut self,
        signal: SignalNumber,
        registry: &mut EventRegistry<Self>,
    ) -> Option<SignalNumber> {
        // Ignore `SIGCONT` while suspending to avoid resuming the terminal twice.
        let sigcont_action = self
            .signal_handler
            .set_action(Signal::SIGCONT, SignalAction::Ignore);

        if let SIGTTOU | SIGTTIN = signal {
            // If sudo is already the foreground process we can resume the command in the
            // foreground. Otherwise, we have to suspend and resume later.
            if !self.foreground && self.check_foreground().is_err() {
                // User's tty was revoked.
                return None;
            }

            if self.foreground {
                dev_info!(
                    "command received {}, parent running in the foreground",
                    signal_fmt(signal)
                );
                if !self.term_raw {
                    if self.tty_pipe.left_mut().set_raw_mode(false).is_ok() {
                        self.term_raw = true;
                    }
                    // Resume command in the foreground
                    return Some(SIGCONT_FG);
                }
            }
        }

        // Stop polling the terminals.
        self.tty_pipe.ignore_events(registry);

        if self.term_raw {
            match self.tty_pipe.left_mut().restore(false) {
                Ok(()) => self.term_raw = false,
                Err(err) => dev_warn!("cannot restore terminal settings: {err}"),
            }
        }

        // FIXME: we should set the action even if we don't have a handler for that signal.
        let saved_signal_action = (signal != SIGSTOP).then_some(signal).and_then(|number| {
            let signal = Signal::from_number(number)?;
            let action = self
                .signal_handler
                .set_action(signal, SignalAction::Default);
            Some((signal, action))
        });

        if self.parent_pgrp != self.sudo_pid && kill(self.parent_pgrp, 0).is_err()
            || killpg(self.parent_pgrp, signal).is_err()
        {
            dev_error!("no parent to suspend, terminating command");
            if let Some(command_pid) = self.command_pid.take() {
                terminate_process(command_pid, true);
            }
        }

        if let Some((signal, action)) = saved_signal_action {
            self.signal_handler.set_action(signal, action);
        }

        if self.command_pid.is_none() || self.resume_terminal().is_err() {
            return None;
        }

        let ret_signal = if self.term_raw {
            SIGCONT_FG
        } else {
            SIGCONT_BG
        };

        self.signal_handler
            .set_action(Signal::SIGCONT, sigcont_action);

        Some(ret_signal)
    }

    /// Check whether we are part of the foreground process group and update the foreground flag.
    fn check_foreground(&mut self) -> io::Result<()> {
        let pgrp = self.tty_pipe.left().tcgetpgrp()?;
        self.foreground = pgrp == self.parent_pgrp;
        Ok(())
    }

    /// Restore the terminal when sudo resumes after receving `SIGCONT`.
    fn resume_terminal(&mut self) -> io::Result<()> {
        self.check_foreground()?;

        // Update the pty settings based on the user's tty.
        self.tty_pipe
            .left()
            .copy_to(self.tty_pipe.right())
            .map_err(|err| {
                dev_error!("cannot copy terminal settings to pty: {err}");
                err
            })?;
        // FIXME: sync the terminal size here.
        dev_info!(
            "parent is in {} ({} -> {})",
            cond_fmt(self.foreground, "foreground", "background"),
            cond_fmt(self.term_raw, "raw", "cooked"),
            cond_fmt(self.foreground, "raw", "cooked"),
        );

        if self.foreground {
            // We're in the foreground, set tty to raw mode.
            if self.tty_pipe.left_mut().set_raw_mode(false).is_ok() {
                self.term_raw = true;
            }
        } else {
            // We're in the background, cannot access tty.
            self.term_raw = false;
        }

        Ok(())
    }

    fn on_signal(&mut self, registry: &mut EventRegistry<Self>) {
        let info = match self.signal_handler.recv() {
            Ok(info) => info,
            Err(err) => {
                dev_error!("parent could not receive signal: {err}");
                return;
            }
        };

        dev_info!(
            "parent received{} {} from {}",
            opt_fmt(info.is_user_signaled(), " user signaled"),
            info.signal(),
            info.pid()
        );

        let Some(monitor_pid) = self.monitor_pid else {
            dev_info!("monitor was terminated, ignoring signal");
            return;
        };

        match info.signal() {
            Signal::SIGCHLD => handle_sigchld(self, registry, "monitor", monitor_pid),
            Signal::SIGCONT => {
                self.resume_terminal().ok();
            }
            // FIXME: check `sync_ttysize`
            Signal::SIGWINCH => {}
            // Skip the signal if it was sent by the user and it is self-terminating.
            _ if info.is_user_signaled() && self.is_self_terminating(info.pid()) => {}
            // FIXME: check `send_command_status`
            signal => self.schedule_signal(signal.number(), registry),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParentEvent {
    Signal,
    Tty(PollEvent),
    Pty(PollEvent),
    Backchannel(PollEvent),
}

impl Process for ParentClosure {
    type Event = ParentEvent;
    type Break = io::Error;
    type Exit = ParentExit;

    fn on_event(&mut self, event: Self::Event, registry: &mut EventRegistry<Self>) {
        match event {
            ParentEvent::Signal => self.on_signal(registry),
            ParentEvent::Tty(poll_event) => {
                self.tty_pipe.on_left_event(poll_event, registry).ok();
            }
            ParentEvent::Pty(poll_event) => {
                self.tty_pipe.on_right_event(poll_event, registry).ok();
            }
            ParentEvent::Backchannel(poll_event) => match poll_event {
                PollEvent::Readable => self.on_message_received(registry),
                PollEvent::Writable => self.check_message_queue(registry),
            },
        }
    }
}

impl HandleSigchld for ParentClosure {
    const OPTIONS: WaitOptions = WaitOptions::new().all().untraced().no_hang();

    fn on_exit(&mut self, _exit_code: c_int, _registry: &mut EventRegistry<Self>) {
        self.monitor_pid = None;
    }

    fn on_term(&mut self, _signal: SignalNumber, _registry: &mut EventRegistry<Self>) {
        self.monitor_pid = None;
    }

    fn on_stop(&mut self, signal: SignalNumber, registry: &mut EventRegistry<Self>) {
        if let Some(signal) = self.suspend_pty(signal, registry) {
            self.schedule_signal(signal, registry);
        }
        self.tty_pipe.resume_events(registry);
    }
}
