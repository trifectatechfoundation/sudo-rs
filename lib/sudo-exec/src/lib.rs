#![deny(unsafe_code)]

mod events;
mod monitor;
mod pty;
mod socket;

use std::{
    cell::RefCell,
    ffi::{c_int, CString, OsStr},
    io,
    os::unix::ffi::OsStrExt,
    os::unix::process::CommandExt,
    process::Command,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use pty::exec_pty;
use signal_hook::{consts::*, low_level::signal_name};
use sudo_common::{context::LaunchType::Login, Context, Environment};
use sudo_log::{user_debug, user_error};
use sudo_system::{interface::ProcessId, kill, killpg, set_target_user, WaitStatus};

/// We only handle the signals that ogsudo handles.
const SIGNALS: &[c_int] = &[
    SIGINT, SIGQUIT, SIGTSTP, SIGTERM, SIGHUP, SIGALRM, SIGPIPE, SIGUSR1, SIGUSR2, SIGCHLD,
    SIGCONT, SIGWINCH,
];

const SIGCONT_FG: c_int = -2;
const SIGCONT_BG: c_int = -3;

enum Never {}

/// Based on `ogsudo`s `exec_pty` function.
pub fn run_command(
    ctx: Context,
    env: Environment,
) -> io::Result<(ExitReason, EmulateDefaultHandler)> {
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
                    user_error!("unable to change directory to {}: {}", path.display(), err);
                    if ctx.chdir.is_some() {
                        return Err(err);
                    }
                }

                Ok(())
            });
        }
    }

    // set target user and groups
    set_target_user(&mut command, ctx.target_user, ctx.target_group);

    let cstat = RefCell::default();
    exec_pty(command, ctx.process.pid as ProcessId, &cstat)
}

/// Atomic type used to decide when to run the default signal handlers
pub type EmulateDefaultHandler = Arc<AtomicBool>;

/// Exit reason for the command executed by sudo.
pub enum ExitReason {
    Code(i32),
    Signal(i32),
}

fn log_wait_status(status: &WaitStatus, process_name: &str) {
    let pid = status.pid();
    if let Some(signal) = status.signaled() {
        let signal = signal_name(signal).unwrap();
        user_debug!("{process_name} ({pid}) terminated by signal {signal}");
    } else if let Some(signal) = status.stopped() {
        let signal = signal_name(signal).unwrap();
        user_debug!("{process_name} ({pid}) stopped by signal {signal}");
    } else if let Some(exit_status) = status.exit_status() {
        user_debug!("{process_name} ({pid}) exited with status {}", exit_status);
    } else {
        user_debug!("{process_name} ({pid}) has continued")
    }
}

fn terminate_command(pid: Option<ProcessId>, use_pgrp: bool) {
    let pid = match pid {
        Some(pid) => pid,
        None => return,
    };

    let kill_fn = if use_pgrp { killpg } else { kill };

    kill_fn(pid, SIGHUP).ok();
    kill_fn(pid, SIGTERM).ok();
    std::thread::sleep(Duration::from_secs(2));
    kill_fn(pid, SIGKILL).ok();
}
