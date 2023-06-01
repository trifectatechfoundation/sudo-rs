use std::{
    cell::RefCell,
    ffi::{c_int, c_ushort},
    io,
    os::fd::{AsRawFd, OwnedFd},
    process::{exit, Command},
};

use signal_hook::{
    consts::*,
    flag::register_conditional_default,
    low_level::{register, signal_name},
};
use sudo_log::{user_debug, user_warn};
use sudo_system::{
    fork, getpgid, getpgrp,
    interface::ProcessId,
    kill, killpg, open, openpty, read,
    signal::{SignalHandler, SignalStream},
    tcgetpgrp,
    term::{self, TermContext},
    waitpid, write, OpenFlags, WaitError, WaitOptions, WaitStatus,
};

use crate::{
    events::EventQueue,
    log_wait_status,
    monitor::exec_monitor,
    socket::{socketpair, Backchannel, CommandStatus},
    terminate_command, EmulateDefaultHandler, ExitReason, SIGCONT_BG, SIGCONT_FG,
};

#[allow(unsafe_code)]
pub(crate) fn exec_pty(
    mut command: Command,
    sudo_pid: ProcessId,
    cstat: &RefCell<CommandStatus>,
) -> io::Result<(ExitReason, EmulateDefaultHandler)> {
    user_debug!("pty::exec_pty");
    user_debug!("sudo pid is {sudo_pid}");
    // FIXME: this needs to be set to the correct value if we ever implement `-b`.
    let background = false;
    let pipeline = false;
    let mut term_raw = false;
    let mut term_ctx = TermContext::new();

    // Allocate a pty if sudo is running in a terminal.
    let (fd_usertty, fd_leader, fd_follower) = pty_setup()?;

    // FIXME: ogsudo registers a cleanup function here by calling `pty_cleanup_init`.

    // We communicate with the monitor using a pair of sockets. Sudo sends signal info to the
    // monitor and the monitor sends back command status updates.
    let (mut sv_0, mut sv_1) = socketpair().expect("unable to create sockets");

    // FIXME: ogsudo allocates an extra socket pair to communicate with `sudo_intercept.so`.

    // We don't wwant to receive SIGTTIN/SIGTTOU
    if let Err(e) = unsafe { register(SIGTTIN, || ()) } {
        user_warn!("unable to set handler for SIGTTIN: {}", e)
    }
    if let Err(e) = unsafe { register(SIGTTOU, || ()) } {
        user_warn!("unable to set handler for SIGTTOU: {}", e)
    }

    // FIXME: ogsudo initializes the policy plugin's session here by calling `policy_init_session`
    // FIXME: ogsudo initializes ttyblock sigset here by calling `init_ttyblock`

    let ppgrp = getpgrp().unwrap_or(-1);

    // Setup the IO streams for the command.
    {
        let clone_follower = || fd_follower.try_clone().expect("cannot clone pty follower");
        command.stdin(clone_follower());
        command.stdout(clone_follower());
        command.stderr(clone_follower());
    }

    let mut events = EventQueue::<ExecClosure>::new();

    if !background {
        // Read from `fd_usertty` and write to `fd_leader`
        // FIXME: ogsudo does this using `io_buf_new`.
        events.add_read_event(&fd_usertty, |ec, _| {
            let mut buf = [0; 6 * 1024];
            let Ok(n) = read(ec.fd_usertty, &mut buf) else { return };
            write(ec.fd_leader, &buf[..n as usize]).ok();
        });
    }

    // Read from `fd_leader` and write to `fd_usertty`
    // FIXME: ogsudo does this using `io_buf_new`.
    events.add_read_event(&fd_leader, |ec, _| {
        let mut buf = [0; 6 * 1024];
        let Ok(n) = read(ec.fd_leader, &mut buf) else { return };
        write(ec.fd_usertty, &buf[..n as usize]).ok();
    });

    // Are we the foreground process?
    let mut foreground = tcgetpgrp(&fd_usertty)
        .map(|pgrp| ppgrp == pgrp)
        .unwrap_or_default();
    user_debug!(
        "sudo is running in the {}",
        if foreground {
            "foreground"
        } else {
            "background"
        }
    );

    // FIXME: ogsudo does some extra setup if any of the IO streams are not a tty and logging is
    // enabled or if sudo is running in background.

    // copy terminal settings from user tty -> pty.  if sudo is a background process, we'll re-init
    // the pty when foregrounded.
    if !term_ctx.copy(&fd_usertty, &fd_leader) {
        user_debug!("unable to copy terminal settings to pty");
        foreground = false;
    }
    user_debug!("copied terminal settings to pty");

    // Start in raw mode unless part of a pipeline.
    if foreground {
        // FIXME: ogsudo does not start in raw mode if it's running in the background.
        if !pipeline && term_ctx.raw(&fd_usertty, 0) {
            user_debug!("/dev/tty set to raw mode");
            term_raw = true;
        }
    }

    // FIXME: ogsudo blocks all incoming signals here. We just "block" whatever we have in `SIGNALS`.
    let emulate_default_handler = EmulateDefaultHandler::default();

    for &signal in super::SIGNALS {
        register_conditional_default(
            signal,
            EmulateDefaultHandler::clone(&emulate_default_handler),
        )
        .ok();
    }

    // FIXME: ogsudo checks if the command terminated earlier here and returns if that's the case.

    let monitor_pid = unsafe { fork() }.expect("unable to fork");
    // child
    if monitor_pid == 0 {
        // FIXME: ogsudo closes the file descriptors for the IO stream pipes here.
        // FIXME: ogsudo removes the cleanup hook here because it should only run in the parent
        // process.
        // FIXME: sudo starts the command in the background if the IO streams are not a tty.
        match exec_monitor(fd_follower, command, foreground, &mut sv_1, cstat) {
            Ok(never) => match never {},
            Err(err) => {
                // If `exec_monitor` returns, it means that executing the monitor failed.
                user_debug!("monitor failed: {err}");
                {
                    let mut cstat = cstat.borrow_mut();
                    *cstat = CommandStatus::from_io_error(&err);
                    if let Err(err) = sv_1.send_status(&cstat) {
                        user_debug!("unable to send status to parent: {}", err);
                    }
                }
                // FIXME: we should close/drop everything before calling `exit`.
                exit(1);
            }
        }
    }

    user_debug!("monitor pid is {monitor_pid}");

    // We close the pty follower here so only the monitor and command have a reference to it.
    drop(fd_follower);

    // Tell the monitor to continue now that the follower is closed.
    *cstat.borrow_mut() = CommandStatus::from_signal(0);
    while let Err(err) = sv_0.send_status(&cstat.borrow()) {
        // FIXME: instead of checking against `11` we should try and check if any `ErrorKind`
        // matches `EAGAIN`
        if err.kind() != io::ErrorKind::Interrupted && err.raw_os_error() != Some(11) {
            panic!("unable to send message to monitor process");
        }
    }
    user_debug!("sent green light to the monitor");

    // Close the socket used by the monitor.
    // FIXME: we should also close the file descriptors that the monitor uses here.
    drop(sv_1);

    // FIXME: ogsudo sets the command timeout here.

    // Fill in the exec closure.
    let mut ec = ExecClosure::new(
        &mut events,
        cstat,
        foreground,
        term_raw,
        monitor_pid,
        sudo_pid,
        ppgrp,
        &mut sv_0,
        &fd_usertty,
        &fd_leader,
        &mut term_ctx,
    );

    // FIXME: ogsudo restores the signal mask and does some addtional setup for IO logging here.

    user_debug!("starting event loop for parent");
    events.start_loop(&mut ec);

    if events.got_break() {
        // error from callback or monitor died
        let cstat = cstat.borrow_mut();
        user_debug!("event loop exited prematurely");
        if cstat.is_invalid() {
            terminate_command(ec.cmnd_pid, true);
            ec.cmnd_pid = None;
            // FIXME: ogsudo sets cstat to WSTATUS with EXITCODE(1, SIGKILL);
        }
    } else {
        // FIXME: ogsudo does some retries if the event loop got `exit`.
    }

    // FIXME: ogsudo flush any remaining output here.

    let cstat = cstat.take();

    // Extracted from `pty_cleanup_int`
    // Restore terminal settings.
    if ec.term_raw {
        // Only restore the terminal if sudo is the foreground process.
        if tcgetpgrp(&fd_usertty).ok() == Some(ec.ppgrp) {
            if ec.term_restore(false) {
                ec.term_raw = false;
            } else {
                user_warn!("unable to restore terminal settings");
            }
        }
    }

    let reason = if let Some(signal) = cstat.signal() {
        ExitReason::Signal(signal)
    } else if let Some(raw) = cstat.wait() {
        let wstat = WaitStatus::from_raw(raw);
        if let Some(signal) = wstat.stopped() {
            ExitReason::Signal(signal)
        } else if let Some(signal) = wstat.signaled() {
            ExitReason::Signal(signal)
        } else if let Some(exit_code) = wstat.exit_status() {
            ExitReason::Code(exit_code)
        } else {
            ExitReason::Code(1)
        }
    } else {
        ExitReason::Code(1)
    };

    Ok((reason, emulate_default_handler))
}

fn pty_setup() -> io::Result<(OwnedFd, OwnedFd, OwnedFd)> {
    user_debug!("pty::pty_setup");
    const PATH_TTY: &str = "/dev/tty";
    let fd_usertty = open(PATH_TTY, OpenFlags::default().read_write()).map_err(|err| {
        user_debug!("no {PATH_TTY}, not allocating a pty");
        err
    })?;
    // FIXME: ogsudo also retrieves the name of the pty and changes its owner using `chown`. This
    // logic is in the `get_pty` function.
    let (fd_leader, fd_follower) = openpty().expect("unable to allocate a pty");

    user_debug!(
        "{PATH_TTY}: fd {}, pty leader fd {}, pty follower fd {}",
        fd_usertty.as_raw_fd(),
        fd_leader.as_raw_fd(),
        fd_follower.as_raw_fd()
    );

    Ok((fd_usertty, fd_leader, fd_follower))
}

struct ExecClosure<'a> {
    backchannel: &'a mut Backchannel,
    sigint_stream: SignalStream<SIGINT>,
    sigquit_stream: SignalStream<SIGQUIT>,
    sigtstp_stream: SignalStream<SIGTSTP>,
    sigterm_stream: SignalStream<SIGTERM>,
    sighup_stream: SignalStream<SIGHUP>,
    sigalrm_stream: SignalStream<SIGALRM>,
    sigusr1_stream: SignalStream<SIGUSR1>,
    sigusr2_stream: SignalStream<SIGUSR2>,
    sigchld_stream: SignalStream<SIGCHLD>,
    sigcont_stream: SignalStream<SIGCONT>,
    sigwinch_stream: SignalStream<SIGWINCH>,
    sigcont_ignore: bool,
    cstat: &'a RefCell<CommandStatus>,
    sudo_pid: ProcessId,
    monitor_pid: Option<ProcessId>,
    cmnd_pid: Option<ProcessId>,
    ppgrp: ProcessId,
    rows: c_ushort,
    cols: c_ushort,
    foreground: bool,
    term_raw: bool,
    term_ctx: &'a mut TermContext,
    fd_usertty: &'a OwnedFd,
    fd_leader: &'a OwnedFd,
}

impl<'a> ExecClosure<'a> {
    fn new(
        events: &mut EventQueue<Self>,
        cstat: &'a RefCell<CommandStatus>,
        foreground: bool,
        term_raw: bool,
        monitor_pid: ProcessId,
        sudo_pid: ProcessId,
        ppgrp: ProcessId,
        backchannel: &'a mut Backchannel,
        fd_usertty: &'a OwnedFd,
        fd_leader: &'a OwnedFd,
        term_ctx: &'a mut TermContext,
    ) -> Self {
        user_debug!("pty::ExecClosure::new");
        let mut ec = Self {
            backchannel,
            sigint_stream: SignalStream::new().unwrap(),
            sigquit_stream: SignalStream::new().unwrap(),
            sigtstp_stream: SignalStream::new().unwrap(),
            sigterm_stream: SignalStream::new().unwrap(),
            sighup_stream: SignalStream::new().unwrap(),
            sigalrm_stream: SignalStream::new().unwrap(),
            sigusr1_stream: SignalStream::new().unwrap(),
            sigusr2_stream: SignalStream::new().unwrap(),
            sigchld_stream: SignalStream::new().unwrap(),
            sigcont_stream: SignalStream::new().unwrap(),
            sigwinch_stream: SignalStream::new().unwrap(),
            sigcont_ignore: false,
            cstat,
            sudo_pid,
            monitor_pid: Some(monitor_pid),
            cmnd_pid: None,
            ppgrp,
            // FIXME: ogsudo sets the rows and cols here using the command details.
            rows: 0,
            cols: 0,
            foreground,
            term_raw,
            term_ctx,
            fd_usertty,
            fd_leader,
        };

        events.add_read_event(&ec.backchannel.as_raw_fd(), |ec, events| {
            ec.check_backchannel(events)
        });

        macro_rules! add_signal_events {
            ($($signo:ident,)*) => {
                $({
                    let stream = ec.get_signal_stream_mut::<$signo>();
                    events.add_read_event(stream, |ec, events| ec.signal_callback::<$signo>(events));
                })*
            };
        }

        add_signal_events!(
            SIGINT, SIGQUIT, SIGTSTP, SIGTERM, SIGHUP, SIGALRM, SIGUSR1, SIGUSR2, SIGCHLD,
            SIGWINCH,
        );

        events.add_read_event(&ec.sigcont_stream, |ec, events| {
            if !ec.sigcont_ignore {
                ec.signal_callback::<SIGCONT>(events)
            } else {
                user_debug!("ignoring SIGCONT");
                ec.sigcont_stream.recv().ok();
            }
        });

        ec
    }

    fn get_signal_stream_mut<const SIGNO: c_int>(&mut self) -> &mut SignalStream<SIGNO> {
        let ptr: *mut SignalStream<SIGNO> = match SIGNO {
            SIGINT => (&mut self.sigint_stream) as *mut _ as _,
            SIGQUIT => (&mut self.sigquit_stream) as *mut _ as _,
            SIGTSTP => (&mut self.sigtstp_stream) as *mut _ as _,
            SIGTERM => (&mut self.sigterm_stream) as *mut _ as _,
            SIGHUP => (&mut self.sighup_stream) as *mut _ as _,
            SIGALRM => (&mut self.sigalrm_stream) as *mut _ as _,
            SIGUSR1 => (&mut self.sigusr1_stream) as *mut _ as _,
            SIGUSR2 => (&mut self.sigusr2_stream) as *mut _ as _,
            SIGCHLD => (&mut self.sigchld_stream) as *mut _ as _,
            SIGCONT => (&mut self.sigcont_stream) as *mut _ as _,
            SIGWINCH => (&mut self.sigwinch_stream) as *mut _ as _,
            _ => unreachable!(),
        };

        #[allow(unsafe_code)]
        unsafe {
            &mut *ptr
        }
    }

    /// Based on `backchannel_cb`send
    fn check_backchannel(&mut self, events: &mut EventQueue<ExecClosure>) {
        user_debug!("pty::ExecClosure::check_backchannel");
        match self.backchannel.receive_status() {
            Err(err) => {
                // FIXME: instead of checking against `11` we should try and check if any `ErrorKind`
                // matches `EAGAIN`
                if err.kind() == io::ErrorKind::Interrupted || err.raw_os_error() == Some(11) {
                    return;
                }
                let mut cstat = self.cstat.borrow_mut();
                if cstat.is_invalid() {
                    *cstat = CommandStatus::from_io_error(&err);
                    events.set_break();
                }
            }
            Ok(cstat) => {
                // Check for command status
                if let Some(pid) = cstat.command_pid() {
                    self.cmnd_pid = Some(pid);
                    user_debug!("executed command, pid {}", pid);
                } else if let Some(raw_status) = cstat.wait() {
                    let status = WaitStatus::from_raw(raw_status);
                    if let Some(signal) = status.stopped() {
                        // Suspend parent and tell monitor how to resume on return;
                        user_debug!("command stopped, suspending parent");
                        let signal = self.suspend(signal);
                        self.schedule_signal(signal, events);
                        // FIXME: ogsudo reenables IO events here.
                    } else {
                        // Command exited or was killed, either way we are done.
                        user_debug!("command exited or was killed");
                        *self.cstat.borrow_mut() = cstat;
                        events.set_exit();
                    }
                } else if let Some(raw_err) = cstat.monitor_err() {
                    // Monitor was unable to execute command
                    user_debug!("errno from monitor: {raw_err}");
                    *self.cstat.borrow_mut() = cstat;
                    events.set_break();
                }
            }
        }
    }
    /// Suspend sudo if the underlying command is suspended. Returns SIGCONT_FG if the command
    /// should be resumed in the foreground or SIGCONT_BG if it is a background process.
    fn suspend(&mut self, signal: c_int) -> c_int {
        user_debug!("pty::ExecClosure::suspend");
        let mut ret = 0;
        // Ignore SIGCONT here to avoid calling resume_terminal multiple times.
        // FIXME: ogsudo does this by calling `sudo_sigaction`.
        let sigcont_handler = self
            .get_signal_stream_mut::<SIGCONT>()
            .set_handler(SignalHandler::Ignore);

        match signal {
            SIGTTIN | SIGTTOU => {
                // If sudo is already the foreground process, just resume the command in the
                // foreground. If not, we'll suspend sudo and resume later.
                if !self.foreground {
                    if self.check_foreground().is_err() {
                        // User's tty was revoked.
                        return ret;
                    }
                } else {
                    user_debug!(
                        "command received {}, parent running in the foreground",
                        signal_name(signal).unwrap()
                    );
                    if !self.term_raw {
                        if self.term_raw(0) {
                            self.term_raw = true;
                        }
                        ret = SIGCONT_FG;
                    }
                }
            }
            // This also covers SIGSTOP and SIGTSTP
            _ => {
                // FIXME: ogsudo deschedules the IO events here.

                // Restore original tty mode before suspending
                if self.term_raw {
                    if self.term_restore(false) {
                        self.term_raw = false;
                    } else {
                        user_warn!("unable to restore terminal settings");
                    }
                }

                // FIXME: ogsudo logs the suspend event here.

                // Suspend self and continue command when we resume
                match signal {
                    SIGINT => {
                        self.sigint_stream.set_handler(SignalHandler::Default);
                    }
                    SIGQUIT => {
                        self.sigquit_stream.set_handler(SignalHandler::Default);
                    }
                    SIGTSTP => {
                        self.sigtstp_stream.set_handler(SignalHandler::Default);
                    }
                    SIGTERM => {
                        self.sigterm_stream.set_handler(SignalHandler::Default);
                    }
                    SIGHUP => {
                        self.sighup_stream.set_handler(SignalHandler::Default);
                    }
                    SIGALRM => {
                        self.sigalrm_stream.set_handler(SignalHandler::Default);
                    }
                    SIGUSR1 => {
                        self.sigusr1_stream.set_handler(SignalHandler::Default);
                    }
                    SIGUSR2 => {
                        self.sigusr2_stream.set_handler(SignalHandler::Default);
                    }
                    SIGCHLD => {
                        self.sigchld_stream.set_handler(SignalHandler::Default);
                    }
                    SIGCONT => {
                        self.sigcont_stream.set_handler(SignalHandler::Default);
                    }
                    SIGWINCH => {
                        self.sigwinch_stream.set_handler(SignalHandler::Default);
                    }
                    // This also covers SIGSTOP
                    _ => {}
                }

                // We stop sudo's process group, even if sudo is not the process group leader. If
                // we only send the signal to sudo itself, the shell will not notice if it is not
                // in monitor mode. THis can happen when sudo is run from a shell script, for
                // example. In this case we need to signal the shell itself. If the process group
                // leader is no longer present, we must kill the command since there will be no one
                // to resume us.
                user_debug!(
                    "killpg({}, {})",
                    self.ppgrp,
                    signal_name(signal).unwrap_or("unknown signal")
                );
                if (self.ppgrp != self.sudo_pid && kill(self.ppgrp, 0).is_err())
                    || killpg(self.ppgrp, signal).is_err()
                {
                    user_debug!("no parent to suspend, terminating command.");
                    terminate_command(self.cmnd_pid, true);
                    self.cmnd_pid = None;
                }

                match signal {
                    SIGINT => {
                        self.sigint_stream.set_handler(SignalHandler::Send);
                    }
                    SIGQUIT => {
                        self.sigquit_stream.set_handler(SignalHandler::Send);
                    }
                    SIGTSTP => {
                        self.sigtstp_stream.set_handler(SignalHandler::Send);
                    }
                    SIGTERM => {
                        self.sigterm_stream.set_handler(SignalHandler::Send);
                    }
                    SIGHUP => {
                        self.sighup_stream.set_handler(SignalHandler::Send);
                    }
                    SIGALRM => {
                        self.sigalrm_stream.set_handler(SignalHandler::Send);
                    }
                    SIGUSR1 => {
                        self.sigusr1_stream.set_handler(SignalHandler::Send);
                    }
                    SIGUSR2 => {
                        self.sigusr2_stream.set_handler(SignalHandler::Send);
                    }
                    SIGCHLD => {
                        self.sigchld_stream.set_handler(SignalHandler::Send);
                    }
                    SIGCONT => {
                        self.sigcont_stream.set_handler(SignalHandler::Ignore);
                    }
                    SIGWINCH => {
                        self.sigwinch_stream.set_handler(SignalHandler::Send);
                    }
                    // This also covers SIGSTOP
                    _ => {}
                }

                // If we failed to suspend, the command is no longer running
                if self.cmnd_pid.is_none() {
                    return ret;
                }
                // FIXME: ogsudo logs the resume event here.

                // Update the pty's terminall settings and restore /dev/tty settings.
                if !self.resume_terminal() {
                    return ret;
                }

                // We always resume the command in the foreground if sudo itself is the foreground
                // process (and we were able to set /dev/tty to raw mode). This helps work around
                // poorly behaved programs that catch SIGTTOU/SIGTTIN but suspend themselves with
                // SIGSTOP. At worst, sudo will go into the background but upon resume the command
                // will be runnable. Otherwise, we can get into a situtation where the command will
                // immediately suspend itself.
                ret = if self.term_raw {
                    SIGCONT_FG
                } else {
                    SIGCONT_BG
                };
            }
        }

        self.get_signal_stream_mut::<SIGCONT>()
            .set_handler(sigcont_handler);

        ret
    }

    // Schedule a signal to be forwarded
    fn schedule_signal(&mut self, signal: c_int, events: &mut EventQueue<ExecClosure>) {
        user_debug!("pty::ExecClosure::schedule_signal");
        if signal == 0 {
            return;
        }

        self.send_command_status(CommandStatus::from_signal(signal), events)
    }

    ///  Based on `send_command_status`.
    fn send_command_status(&mut self, cstat: CommandStatus, events: &mut EventQueue<ExecClosure>) {
        user_debug!("pty::ExecClosure::send_command_status");
        self.handle_send_cstat(cstat, events);
        events.set_continue();
    }

    /// Based on `fwdchannel_cb`
    fn handle_send_cstat(&mut self, cstat: CommandStatus, events: &mut EventQueue<ExecClosure>) {
        user_debug!("pty::ExecClosure::handle_send_cstat");
        user_debug!("Sending {:?} to monitor over backchannel", cstat);
        if let Err(err) = self.backchannel.send_status(&cstat) {
            if err.kind() == io::ErrorKind::BrokenPipe {
                user_debug!("broken pipe writing to monitor over backchannel");
                *self.cstat.borrow_mut() = CommandStatus::from_io_error(&err);
                events.set_break();
            }
        }
    }

    // Check whether we are running in the foregroup.
    // Updates the foreground flag and updates the window size.
    //
    // Returns the foreground proces group ID on success.
    fn check_foreground(&mut self) -> io::Result<ProcessId> {
        user_debug!("pty::ExecClosure::check_foreground");
        let pid = tcgetpgrp(self.fd_usertty)?;
        self.foreground = pid == self.ppgrp;
        Ok(pid)
    }

    fn term_restore(&mut self, flush: bool) -> bool {
        user_debug!("pty::ExecClosure::term_restore");
        self.term_ctx.restore(self.fd_usertty, flush)
    }

    fn term_copy(&mut self) -> bool {
        user_debug!("pty::ExecClosure::term_copy");
        self.term_ctx.copy(self.fd_usertty, self.fd_leader)
    }

    fn term_raw(&mut self, isig: i32) -> bool {
        user_debug!("pty::ExecClosure::term_raw");
        self.term_ctx.raw(self.fd_usertty, isig)
    }

    /// Restore the terminal when sudo is resumed in response to SIGCONT.
    /// Based on `resume_terminal`
    fn resume_terminal(&mut self) -> bool {
        user_debug!("pty::ExecClosure::resume_terminal");
        if self.check_foreground().is_err() {
            // User's tty was revoked.
            return false;
        }
        // update the pty settings based on the user's terminal
        if !self.term_copy() {
            user_debug!("unable to copy terminal settings to pty");
        }
        self.sync_ttysize();

        user_debug!(
            "parent is in {} ({} -> {})",
            if self.foreground {
                "foreground"
            } else {
                "background"
            },
            if self.term_raw { "raw" } else { "cooked" },
            if self.foreground { "raw" } else { "cooked" }
        );

        if self.foreground {
            // Foreground process, set tty to raw mode.
            if self.term_raw(0) {
                self.term_raw = true;
            }
        } else {
            // Background proces, no access to tty.
            self.term_raw = false;
        }

        true
    }

    /// Based on `sync_ttysize`
    fn sync_ttysize(&mut self) {
        user_debug!("pty::ExecClosure::sync_ttysize");
        let Ok(wsize) = term::WinSize::get(self.fd_usertty) else {
            return;
        };

        let rows = wsize.rows();
        let cols = wsize.cols();

        if rows != self.rows || cols != self.cols {
            user_debug!("{} x {} -> {rows} x {cols}", self.rows, self.cols);

            // Update pty window size and send command SIGWINCH.
            wsize.set(self.fd_leader).ok();
            killpg(self.cmnd_pid.unwrap_or(-1), SIGWINCH).ok();

            // Update rows/cols.
            self.rows = rows;
            self.cols = cols;
        }
    }

    /// Based on `signal_cb_pty`
    fn signal_callback<const SIGNO: c_int>(&mut self, events: &mut EventQueue<ExecClosure>) {
        user_debug!("pty::ExecClosure::signal_callback");
        user_debug!(
            "parent received {}",
            signal_name(SIGNO).unwrap_or("unknown signal"),
        );

        if self.monitor_pid.is_none() {
            return;
        }

        let info = self
            .get_signal_stream_mut::<SIGNO>()
            .recv()
            .expect("fd was polled, this should not fail");

        match SIGNO {
            SIGCHLD => self.handle_sigchld(events),
            SIGCONT => {
                self.resume_terminal();
            }
            SIGWINCH => self.sync_ttysize(),
            signal => {
                if info.is_user_signaled() && self.is_self_terminating(info.get_pid()) {
                    return;
                }
                self.schedule_signal(signal, events);
            }
        }
    }
    /// Decides if the signal sent by the `signaler` process is self-terminating.
    ///
    /// A signal is self-terminating if the PID of the `process`:
    /// - is the same PID of the command, or
    /// - is in the process group of the command and either sudo or the command is the leader.
    fn is_self_terminating(&self, signaler_pid: ProcessId) -> bool {
        user_debug!("pty::ExecClosure::is_self_terminating");
        if signaler_pid != 0 {
            if Some(signaler_pid) == self.cmnd_pid {
                return true;
            }

            if let Ok(signaler_pgrp) = getpgid(signaler_pid) {
                if Some(signaler_pgrp) == self.cmnd_pid || signaler_pgrp == self.sudo_pid {
                    return true;
                }
            }
        }

        false
    }

    /// Handle changes to the monitors's status (SIGCHLD).
    /// Based on `handle_sigchld_pty`
    fn handle_sigchld(&mut self, events: &mut EventQueue<ExecClosure>) {
        user_debug!("pty::ExecClosure::handle_sigchld");
        // There may be multiple children in intercept mode.
        // FIXME: do we care about this?
        loop {
            let status = loop {
                match waitpid(None, WaitOptions::default().all().untraced().no_hang()) {
                    Err(err) => match err {
                        WaitError::Unavailable => {}
                        WaitError::Io(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                        WaitError::Io(err) => {
                            // FIXME: we should be able to check if the IO error is `ECHILD` somehow.
                            if err.raw_os_error() != Some(10) {
                                return user_warn!("waitpid");
                            }
                        }
                    },
                    Ok(status) => break status,
                }
                // Nothing left to wait for
                return;
            };

            log_wait_status(&status, "child");

            let pid = status.pid();
            if status.exit_status().is_some() || status.signaled().is_some() {
                if Some(pid) == self.monitor_pid {
                    self.monitor_pid = None;
                }
            } else if let Some(signal) = status.stopped() {
                if Some(pid) != self.monitor_pid {
                    continue;
                }
                // If the monitor dies we get notified via backchannel. If it was stopped, we
                // should stop too (the command keeps running it its pty) and continue it when it
                // comes back.
                user_debug!("monitor stopped, suspending sudo");
                let signal = self.suspend(signal);
                user_debug!("sending SIGCONT to {pid}");
                kill(pid, SIGCONT).ok();
                self.schedule_signal(signal, events);
                // FIXME: ogsudo reenables IO events here.
            } else {
                user_debug!("unexpected wait status for process {pid}");
            }
        }
    }
}
