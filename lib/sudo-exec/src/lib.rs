#![deny(unsafe_code)]

use std::{
    ffi::{c_int, CString, OsStr},
    io,
    os::unix::ffi::OsStrExt,
    os::unix::process::{CommandExt, ExitStatusExt},
    process::{exit, Command, ExitStatus},
    time::Duration,
};

use signal_hook::{
    consts::*,
    iterator::{exfiltrator::WithOrigin, SignalsInfo},
    low_level::{
        emulate_default_handler,
        siginfo::{Cause, Process, Sent},
    },
};
use sudo_common::{context::LaunchType::Login, Context, Environment};
use sudo_log::{auth_warn, user_error, user_warn};
use sudo_system::{
    fork, getpgid, interface::ProcessId, kill, openpty, pipe, read, set_controlling_terminal,
    set_target_user, setpgid, setsid, write,
};

/// We only handle the signals that ogsudo handles.
const SIGNALS: &[c_int] = &[
    SIGINT, SIGQUIT, SIGTSTP, SIGTERM, SIGHUP, SIGALRM, SIGPIPE, SIGUSR1, SIGUSR2, SIGCHLD,
    SIGCONT, SIGWINCH,
];

/// Based on `ogsudo`s `exec_pty` function.
pub fn run_command(ctx: Context, env: Environment) -> io::Result<ExitStatus> {
    // FIXME: should we pipe the stdio streams?
    let mut command = Command::new(&ctx.command.command);
    // reset env and set filtered environment
    command.args(ctx.command.arguments).env_clear().envs(env);
    // Decide if the pwd should be changed. `--chdir` takes precedence over `-i`.
    let path = ctx.chdir.as_ref().or_else(|| {
        (ctx.launch == Login).then(|| {
            // signal to the operating system that the command is a login shell by prefixing "-"
            let mut process_name = ctx
                .command
                .command
                .file_name()
                .map(|osstr| osstr.as_bytes().to_vec())
                .unwrap_or_else(Vec::new);
            process_name.insert(0, b'-');
            command.arg0(OsStr::from_bytes(&process_name));

            &ctx.target_user.home
        })
    });

    // change current directory if necessary.
    if let Some(path) = path.cloned() {
        #[allow(unsafe_code)]
        unsafe {
            command.pre_exec(move || {
                let bytes = path.as_os_str().as_bytes();

                let c_path =
                    CString::new(bytes).expect("nul byte found in provided directory path");

                if let Err(err) = sudo_system::chdir(&c_path) {
                    if ctx.chdir.is_some() {
                        user_error!("unable to change directory to {}: {}", path.display(), err);
                        return Err(err);
                    } else {
                        user_warn!("unable to change directory to {}: {}", path.display(), err);
                    }
                }

                Ok(())
            });
        }
    }

    // set target user and groups
    set_target_user(&mut command, ctx.target_user, ctx.target_group);

    // FIXME: Look for `SFD_LEADER` occurences in `exec_pty` to decide what to do with the leader
    // side of the pty. It should be used to handle signals like `SIGWINCH` and `SIGCONT`.
    // FIXME: close this!
    let (_pty_leader, pty_follower) = openpty()?;

    // FIXME: close this!
    let (rx, tx) = pipe()?;

    // FIXME: fork sucks. Find a better abstraction.
    let monitor_pid = fork()?;
    // Monitor logic. Based on `exec_monitor`. It is very important that the content of this block
    // diverges so the monitor doesn't execute code it shouldn't.
    if monitor_pid == 0 {
        // Create new terminal session.
        setsid()?;

        // Set the pty as the controlling terminal.
        set_controlling_terminal(pty_follower)?;

        // spawn and exec to command
        let mut command = command.spawn()?;

        let command_pid = command.id() as ProcessId;

        // set the process group ID of the command to the command PID.
        let command_pgrp = command_pid;
        setpgid(command_pid, command_pgrp);

        let mut signals = SignalsInfo::<WithOrigin>::new(SIGNALS)?;

        // FIXME: There should be a nice abstraction for these loops that wait for something to
        // happen and handle signals meanwhile.
        loop {
            // First we check if the command is finished
            if let Some(status) = command.try_wait()? {
                write(tx, &status.into_raw().to_ne_bytes())?;

                // Given that we overwrote the default handlers for all the signals, we musti
                // emulate them to handle the signal we just sent correctly.
                for info in signals.pending() {
                    emulate_default_handler(info.signal)?;
                }

                // We exit because we don't have anything else to do as a monitor.
                exit(0);
            }

            // Then we check any pending signals that we received. Based on `mon_signal_cb`
            for info in signals.pending() {
                let user_signaled = info.cause == Cause::Sent(Sent::User);
                match info.signal {
                    SIGCHLD => {
                        // FIXME: check `mon_handle_sigchld`
                        // We just wait until all the children are done.
                        continue;
                    }
                    _ => {
                        // Skip the signal if it was sent by the user and it is self-terminating.
                        if user_signaled
                            && is_self_terminating_mon(info.process, command_pid, command_pgrp)
                        {
                            continue;
                        }
                    }
                }

                let status = if info.signal == SIGALRM {
                    // Kill the command with increasing urgency.
                    // Based on `terminate_command`.
                    kill(command_pid, SIGHUP);
                    kill(command_pid, SIGTERM);
                    std::thread::sleep(Duration::from_secs(2));
                    kill(command_pid, SIGKILL)
                } else {
                    kill(command_pid, info.signal)
                };

                if status != 0 {
                    eprintln!("kill failed");
                }
            }
        }
    }

    let mut buf = 0i32.to_ne_bytes();

    let mut signals = SignalsInfo::<WithOrigin>::new(SIGNALS)?;

    loop {
        // First we check if the monitor sent us the exit status of the command.
        if read(rx, &mut buf).is_ok() {
            let status = ExitStatus::from_raw(i32::from_ne_bytes(buf));

            if let Some(signal) = status.signal() {
                // If the command terminated because of a signal, we send this signal to sudo
                // itself to match the original sudo behavior. If we fail we just return the status
                // code.
                if kill(ctx.process.pid, signal) != -1 {
                    // Given that we overwrote the default handlers for all the signals, we musti
                    // emulate them to handle the signal we just sent correctly.
                    for info in signals.pending() {
                        emulate_default_handler(info.signal)?;
                    }
                }
            }

            return Ok(status);
        }

        // Then we check any pending signals that we received. Based on `signal_cb_pty`
        for info in signals.pending() {
            let user_signaled = info.cause == Cause::Sent(Sent::User);
            match info.signal {
                SIGCHLD => {
                    // FIXME: check `handle_sigchld_pty`
                    // We just wait until all the children are done.
                    continue;
                }
                SIGCONT => {
                    // FIXME: check `resume_terminal`
                    continue;
                }
                SIGWINCH => {
                    // FIXME: check `sync_ttysize`
                    continue;
                }
                _ => {
                    // Skip the signal if it was sent by the user and it is self-terminating.
                    if user_signaled && is_self_terminating_pty(info.process, -1, ctx.process.pid) {
                        continue;
                    }
                }
            }

            // FIXME: check `send_command_status`
            if kill(monitor_pid, info.signal) != 0 {
                user_error!("kill failed");
            }
        }
    }
}

/// Decides if the signal sent by the `signaler` process is self-terminating.
///
/// A signal is self-terminating if the PID of the `process`:
/// - is the same PID of the command, or
/// - is in the process group of the command and the command is the leader.
fn is_self_terminating_mon(
    signaler: Option<Process>,
    command_pid: ProcessId,
    command_prgp: ProcessId,
) -> bool {
    if let Some(signaler) = signaler {
        if signaler.pid != 0 {
            if signaler.pid == command_pid {
                return true;
            }
            let grp_leader = getpgid(signaler.pid);

            if grp_leader != -1 {
                if grp_leader == command_prgp {
                    return true;
                }
            } else {
                eprintln!("Could not fetch process group ID");
            }
        }
    }

    false
}

/// Decides if the signal sent by the `signaler` process is self-terminating.
///
/// A signal is self-terminating if the PID of the `process`:
/// - is the same PID of the command, or
/// - is in the process group of the command and either sudo or the command is the leader.
fn is_self_terminating_pty(
    signaler: Option<Process>,
    command_pid: ProcessId,
    sudo_pid: ProcessId,
) -> bool {
    if let Some(signaler) = signaler {
        if signaler.pid != 0 {
            if signaler.pid == command_pid {
                return true;
            }

            let signaler_pgrp = getpgid(signaler.pid);
            if signaler_pgrp != -1 {
                if signaler_pgrp == command_pid || signaler_pgrp == sudo_pid {
                    return true;
                }
            } else {
                auth_warn!("Could not fetch process group ID");
            }
        }
    }

    false
}
