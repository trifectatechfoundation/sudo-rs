use std::ffi::c_int;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::exit;
use std::time::Duration;
use std::{cell::RefCell, io, os::fd::OwnedFd, process::Command};

use signal_hook::consts::*;

use signal_hook::low_level::signal_name;
use sudo_log::{user_debug, user_warn};
use sudo_system::interface::ProcessId;
use sudo_system::signal::SignalStream;
use sudo_system::{
    fork, getpgid, getpgrp, killpg, pipe, read, set_controlling_terminal, setpgid, setsid,
    tcgetpgrp, tcsetpgrp, waitpid, write, WaitError, WaitOptions,
};

use crate::events::EventQueue;
use crate::socket::{Backchannel, CommandStatus};
use crate::{log_wait_status, terminate_command, Never, SIGCONT_BG, SIGCONT_FG};

pub(crate) fn exec_monitor(
    fd_follower: OwnedFd,
    command: Command,
    foreground: bool,
    backchannel: &mut Backchannel,
    cstat: &RefCell<CommandStatus>,
) -> io::Result<Never> {
    user_debug!("monitor::exec_monitor");
    // FIXME: ogsudo closes here the FDs that the monitor won't use.
    // FIXME: ogsudo ignores SIGTTIN and SIGTTOU here but those shouldn't be possible to receive.

    // Start a new session with the parent as the session leader and the follower as the
    // controlling terminal.
    setsid().map_err(|err| {
        user_warn!("setsid: {}", err);
        err
    })?;

    set_controlling_terminal(&fd_follower).map_err(|err| {
        user_warn!("unable to set controlling tty: {}", err);
        err
    })?;

    // We use a pipe to get errno if exec fails in the child.
    let (errpipe_0, errpipe_1) = pipe().map_err(|err| {
        user_warn!("unable to create pipe: {err}");
        err
    })?;

    // Wait for the main sudo process to give us green light before spawning the command. This
    // avoids race conditions when the command exits quickly.
    loop {
        match backchannel.receive_status() {
            Ok(new_cstat) => {
                user_debug!("received green light from parent");
                break *cstat.borrow_mut() = new_cstat;
            }
            Err(err) => {
                // FIXME: instead of checking against `11` we should try and check if any `ErrorKind`
                // matches `EAGAIN`
                if err.kind() != io::ErrorKind::Interrupted && err.raw_os_error() != Some(11) {
                    user_warn!("unable to receive message from parent");
                    return Err(err);
                }
            }
        }
    }

    // FIXME: ogsudo does some extra config if selinux is available here.

    #[allow(unsafe_code)]
    let cmnd_pid = unsafe { fork() }.map_err(|err| {
        user_warn!("unable to fork: {err}");
        err
    })?;

    if cmnd_pid == 0 {
        // child
        drop(backchannel);
        drop(errpipe_0);

        // setup tty and exec command
        let err = exec_cmnd(command, foreground, fd_follower);

        if write(&errpipe_1, &err.raw_os_error().unwrap_or(-1).to_ne_bytes()).is_err() {
            user_warn!("unable to execute command: {}", err);
        }

        exit(1);
    }

    user_debug!("command pid is {cmnd_pid}");

    drop(errpipe_1);

    // Send the command's PID to the main sudo process.
    {
        let mut cstat = cstat.borrow_mut();
        *cstat = CommandStatus::from_pid(cmnd_pid);
        send_status(backchannel, &mut *cstat).ok();
    }

    let mut events = EventQueue::<ExecClosure>::new();

    let mut mc = ExecClosure::new(
        &mut events,
        cmnd_pid,
        cstat,
        &fd_follower,
        backchannel,
        &errpipe_0,
    );

    // Put command in its own process group.
    setpgid(cmnd_pid, mc.cmnd_pgrp).ok();

    // Set the command as the foreground process for the pty follower.
    if foreground {
        if let Err(err) = tcsetpgrp(&fd_follower, mc.cmnd_pgrp) {
            user_debug!(
                "unable to set foreground pgrp to {} (command): {err}",
                mc.cmnd_pgrp
            );
        }
    }

    cstat.borrow_mut().take();

    user_debug!("starting event loop for monitor");
    events.start_loop(&mut mc);

    if mc.cmnd_pid.is_some() {
        // Command still running, did the parent die?
        user_debug!("Command still running after event loop exit, terminating");
        terminate_command(mc.cmnd_pid, true);
        while match waitpid(mc.cmnd_pid, WaitOptions::default()) {
            Err(WaitError::Io(err)) => err.kind() == io::ErrorKind::Interrupted,
            _ => false,
        } {}
    }

    // Take the controlling tty. This prevent processes spawned by the command from receiving
    // SIGHUP when the session leader (us) exits.
    if let Err(err) = tcsetpgrp(&fd_follower, mc.mon_pgrp) {
        user_debug!(
            "unable to set foreground pgrp to {} (monitor): {err}",
            mc.mon_pgrp
        );
    }

    send_status(backchannel, &mut *cstat.borrow_mut()).ok();

    // FIXME: ogsudo does some extra config if selinux is available here.

    exit(1)
}

fn exec_cmnd(mut command: Command, foreground: bool, fd_follower: OwnedFd) -> io::Error {
    user_debug!("monitor::exec_cmnd");
    let cmnd_pid = std::process::id() as ProcessId;
    // Set command process group here too to avoid a race.
    setpgid(0, cmnd_pid).ok();

    // FIXME: ogsudo wires up the IO streams here.

    // Wait for parent to grant us the tty if we are foreground
    if foreground {
        user_debug!("waiting for controlling tty");
        loop {
            match tcgetpgrp(&fd_follower) {
                Ok(pid) if pid == cmnd_pid => break user_debug!("got controlling tty"),
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    }

    // Done with the pty follower, don't leak it.
    drop(fd_follower);

    user_debug!(
        "executing command in the {}",
        if foreground {
            "foreground"
        } else {
            "background"
        }
    );
    command.exec()
}

struct ExecClosure<'a> {
    sigint_recv: SignalStream<SIGINT>,
    sigquit_recv: SignalStream<SIGQUIT>,
    sigtstp_recv: SignalStream<SIGTSTP>,
    sigterm_recv: SignalStream<SIGTERM>,
    sighup_recv: SignalStream<SIGHUP>,
    sigalrm_recv: SignalStream<SIGALRM>,
    sigusr1_recv: SignalStream<SIGUSR1>,
    sigusr2_recv: SignalStream<SIGUSR2>,
    sigchld_recv: SignalStream<SIGCHLD>,
    sigcont_recv: SignalStream<SIGCONT>,
    sigwinch_recv: SignalStream<SIGWINCH>,
    cstat: &'a RefCell<CommandStatus>,
    cmnd_pid: Option<ProcessId>,
    cmnd_pgrp: ProcessId,
    mon_pgrp: ProcessId,
    backchannel: &'a mut Backchannel,
    fd_follower: &'a OwnedFd,
    errfd: &'a OwnedFd,
}

impl<'a> ExecClosure<'a> {
    fn new(
        events: &mut EventQueue<Self>,
        cmnd_pid: ProcessId,
        cstat: &'a RefCell<CommandStatus>,
        fd_follower: &'a OwnedFd,
        backchannel: &'a mut Backchannel,
        errfd: &'a OwnedFd,
    ) -> Self {
        user_debug!("monitor::ExecClosure::new");
        let mon_pgrp = getpgrp().unwrap_or(-1);

        let mc = Self {
            cstat,
            sigint_recv: SignalStream::new().unwrap(),
            sigquit_recv: SignalStream::new().unwrap(),
            sigtstp_recv: SignalStream::new().unwrap(),
            sigterm_recv: SignalStream::new().unwrap(),
            sighup_recv: SignalStream::new().unwrap(),
            sigalrm_recv: SignalStream::new().unwrap(),
            sigusr1_recv: SignalStream::new().unwrap(),
            sigusr2_recv: SignalStream::new().unwrap(),
            sigchld_recv: SignalStream::new().unwrap(),
            sigcont_recv: SignalStream::new().unwrap(),
            sigwinch_recv: SignalStream::new().unwrap(),
            cmnd_pid: Some(cmnd_pid),
            cmnd_pgrp: cmnd_pid,
            mon_pgrp,
            fd_follower,
            backchannel,
            errfd,
        };

        events.add_read_event(&mc.backchannel.as_raw_fd(), |mc, events| {
            mc.check_backchannel(events)
        });

        events.add_read_event(&mc.sigint_recv.as_raw_fd(), |mc, events| {
            mc.handle_recv_signal(SIGINT, events)
        });
        events.add_read_event(&mc.sigquit_recv.as_raw_fd(), |mc, events| {
            mc.handle_recv_signal(SIGQUIT, events)
        });
        events.add_read_event(&mc.sigtstp_recv.as_raw_fd(), |mc, events| {
            mc.handle_recv_signal(SIGTSTP, events)
        });
        events.add_read_event(&mc.sigterm_recv.as_raw_fd(), |mc, events| {
            mc.handle_recv_signal(SIGTERM, events)
        });
        events.add_read_event(&mc.sighup_recv.as_raw_fd(), |mc, events| {
            mc.handle_recv_signal(SIGHUP, events)
        });
        events.add_read_event(&mc.sigalrm_recv.as_raw_fd(), |mc, events| {
            mc.handle_recv_signal(SIGALRM, events)
        });
        events.add_read_event(&mc.sigusr1_recv.as_raw_fd(), |mc, events| {
            mc.handle_recv_signal(SIGUSR1, events)
        });
        events.add_read_event(&mc.sigusr2_recv.as_raw_fd(), |mc, events| {
            mc.handle_recv_signal(SIGUSR2, events)
        });
        events.add_read_event(&mc.sigchld_recv.as_raw_fd(), |mc, events| {
            mc.handle_recv_signal(SIGCHLD, events)
        });
        events.add_read_event(&mc.sigcont_recv.as_raw_fd(), |mc, events| {
            mc.handle_recv_signal(SIGCONT, events)
        });
        events.add_read_event(&mc.sigwinch_recv.as_raw_fd(), |mc, events| {
            mc.handle_recv_signal(SIGWINCH, events)
        });

        events.add_read_event(&mc.errfd.as_raw_fd(), |mc, events| mc.check_errpipe(events));

        mc
    }

    /// Based on `mon_backchannel_cb`
    fn check_backchannel(&mut self, events: &mut EventQueue<ExecClosure>) {
        user_debug!("monitor::ExecClosure::check_backchannel");
        match self.backchannel.receive_status() {
            Err(err) => {
                // FIXME: instead of checking against `11` we should try and check if any `ErrorKind`
                // matches `EAGAIN`
                if err.kind() == io::ErrorKind::Interrupted || err.raw_os_error() == Some(11) {
                    return;
                }
                user_warn!("error reading from socketpair: {}", err);
                events.set_break()
            }
            Ok(cstat) => {
                if let Some(signal) = cstat.signal() {
                    self.deliver_signal(signal, true);
                } else {
                    user_warn!("unexpected reply type on backchannel: {:?}", cstat);
                }
            }
        }
    }

    /// Based on `mon_signal_cb`
    fn handle_recv_signal(&mut self, signal: c_int, events: &mut EventQueue<ExecClosure>) {
        user_debug!("monitor::ExecClosure::handle_signal");
        user_debug!(
            "monitor received {}",
            signal_name(signal).unwrap_or("unknown signal"),
        );

        let info = match signal {
            SIGINT => self.sigint_recv.recv().unwrap(),
            SIGQUIT => self.sigquit_recv.recv().unwrap(),
            SIGTSTP => self.sigtstp_recv.recv().unwrap(),
            SIGTERM => self.sigterm_recv.recv().unwrap(),
            SIGHUP => self.sighup_recv.recv().unwrap(),
            SIGALRM => self.sigalrm_recv.recv().unwrap(),
            SIGUSR1 => self.sigusr1_recv.recv().unwrap(),
            SIGUSR2 => self.sigusr2_recv.recv().unwrap(),
            SIGCHLD => self.sigchld_recv.recv().unwrap(),
            SIGCONT => self.sigcont_recv.recv().unwrap(),
            SIGWINCH => self.sigwinch_recv.recv().unwrap(),
            _ => unreachable!(),
        };

        if signal == SIGCHLD {
            self.handle_sigchld();
            if self.cmnd_pid.is_none() {
                // Command exited or was killed, exit event loop
                events.set_exit();
            }
        } else {
            if info.is_user_signaled() && self.is_self_terminating(info.get_pid()) {
                return;
            }
            self.deliver_signal(signal, false);
        }
    }

    fn check_errpipe(&mut self, events: &mut EventQueue<ExecClosure>) {
        user_debug!("monitor::ExecClosure::check_errpipe");
        let mut buf = (0 as c_int).to_ne_bytes();

        if let Err(err) = read(self.errfd, &mut buf) {
            if err.raw_os_error() != Some(11) && err.kind() != io::ErrorKind::Interrupted {
                let mut cstat = self.cstat.borrow_mut();
                if cstat.is_invalid() {
                    *cstat = CommandStatus::from_io_error(&err);
                }
                user_debug!("failed to read error pipe: {err}");
                events.set_break();
            }
        } else {
            let raw = c_int::from_ne_bytes(buf);
            user_debug!("errno from child: {raw}");
            *self.cstat.borrow_mut() =
                CommandStatus::from_io_error(&io::Error::from_raw_os_error(raw));
        }
    }

    /// Decides if the signal sent by the `signaler` process is self-terminating.
    ///
    /// A signal is self-terminating if the PID of the `signaler`:
    /// - is the same PID of the command, or
    /// - is in the process group of the command and the command is the leader.
    fn is_self_terminating(&self, signaler_pid: ProcessId) -> bool {
        user_debug!("monitor::ExecClosure::is_self_terminating");
        if signaler_pid != 0 {
            if Some(signaler_pid) == self.cmnd_pid {
                return true;
            }

            if let Ok(grp_leader) = getpgid(signaler_pid) {
                if grp_leader == self.cmnd_pgrp {
                    return true;
                }
            }
        }

        false
    }
    /// Deliver a signal to the running command.
    /// The signal was either forwarded to us by the parent sudo process or was received by the
    /// monitor itself.
    ///
    /// There are special signals, SIGCONT_BG and SIGCONT_FG which specify whether the command
    /// should have the controlling tty.
    fn deliver_signal(&self, signal: c_int, from_parent: bool) {
        user_debug!("monitor::ExecClosure::deliver_signal");
        // Avoid killing more than a single process or process group
        let Some(cmnd_pid) = self.cmnd_pid else {
            return;
        };

        user_debug!(
            "monitor received {}{}",
            signal_name(signal).unwrap_or_else(|| match signal {
                SIGCONT_FG => "SIGCONT_FG",
                SIGCONT_BG => "SIGCONT_BG",
                _ => "unknown signal",
            }),
            if from_parent { " from parent" } else { "" }
        );

        // Handle signal from parent or monitor
        match signal {
            SIGALRM => {
                terminate_command(Some(cmnd_pid), true);
            }
            SIGCONT_FG => {
                // Continue in foreground, grant it controlling tty.
                if let Err(err) = tcsetpgrp(self.fd_follower, self.cmnd_pgrp) {
                    user_debug!(
                        "unable to set foreground pgrp to {} (command): {err}",
                        self.cmnd_pgrp
                    );
                }
                killpg(cmnd_pid, SIGCONT).ok();
            }
            SIGCONT_BG => {
                // Continue in background, I take controlling tty.
                if let Err(err) = tcsetpgrp(self.fd_follower, self.mon_pgrp) {
                    user_debug!(
                        "unable to set foreground pgrp to {} (monitor): {err}",
                        self.mon_pgrp
                    );
                }
                killpg(cmnd_pid, SIGCONT).ok();
            }
            SIGKILL => unreachable!(),
            _ => {
                // Relay signal to command.
                killpg(cmnd_pid, signal).ok();
            }
        }
    }

    /// Based on `mon_handle_sigchld`.
    fn handle_sigchld(&mut self) {
        user_debug!("monitor::ExecClosure::handle_sigchld");
        // Read command status
        let status = loop {
            match waitpid(self.cmnd_pid, WaitOptions::default().untraced().no_hang()) {
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
            // Nothing to wait for
            return user_debug!("no process to wait for");
        };

        log_wait_status(&status, "command");

        let wstatus = CommandStatus::from(status.clone());
        let mut cstat = self.cstat.borrow_mut();
        // Be sure we don't overwrite the `spawn` error with the child exit status
        if cstat.is_invalid() {
            // Store the wait status of the command on `cstat` and forward it to the parent if
            // stopped.
            *cstat = wstatus;
            if status.stopped().is_some() {
                // Save the foreground pgid so we can restore it later.
                let pid = tcgetpgrp(self.fd_follower).unwrap_or(-1);
                if pid != self.mon_pgrp {
                    self.cmnd_pgrp = pid;
                }
                send_status(self.backchannel, &mut *cstat).ok();
            }
        } else {
            user_debug!(
                "not overwritting command status {:?} with {:?}",
                cstat,
                wstatus
            );
        }
    }
}

fn send_status(backchannel: &mut Backchannel, cstat: &mut CommandStatus) -> io::Result<()> {
    user_debug!("monitor::send_status");
    if cstat.is_invalid() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "command status is invalid and cannot be sent",
        ));
    }

    backchannel.send_status(&cstat.take())
}
