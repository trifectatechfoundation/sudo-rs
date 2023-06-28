#![deny(unsafe_code)]

mod event;
mod interface;
mod io_util;
mod no_pty;
mod signal_manager;
mod use_pty;

use std::{
    borrow::Cow,
    ffi::{CString, OsStr},
    io,
    os::unix::ffi::OsStrExt,
    os::unix::process::CommandExt,
    process::Command,
    time::Duration,
};

use signal_hook::consts::*;

use crate::{
    common::Environment,
    system::{interface::ProcessId, killpg},
};
use crate::{
    exec::no_pty::exec_no_pty,
    log::dev_info,
    system::{set_target_user, signal::SignalNumber, term::UserTerm},
};
use crate::{log::user_error, system::kill};

pub use interface::RunOptions;

use self::use_pty::{exec_pty, SIGCONT_BG, SIGCONT_FG};

/// Based on `ogsudo`s `exec_pty` function.
///
/// Returns the [`ExitReason`] of the command and a function that restores the default handler for
/// signals once its called.
pub fn run_command(
    options: &impl RunOptions,
    env: Environment,
) -> io::Result<(ExitReason, impl FnOnce())> {
    // FIXME: should we pipe the stdio streams?
    let qualified_path = options.command()?;
    let mut command = Command::new(qualified_path);
    // reset env and set filtered environment
    command.args(options.arguments()).env_clear().envs(env);
    // Decide if the pwd should be changed. `--chdir` takes precedence over `-i`.
    let path = options.chdir().cloned().or_else(|| {
        options.is_login().then(|| {
            // signal to the operating system that the command is a login shell by prefixing "-"
            let mut process_name = qualified_path
                .file_name()
                .map(|osstr| osstr.as_bytes().to_vec())
                .unwrap_or_else(Vec::new);
            process_name.insert(0, b'-');
            command.arg0(OsStr::from_bytes(&process_name));

            options.user().home.clone()
        })
    });

    // set target user and groups
    set_target_user(
        &mut command,
        options.user().clone(),
        options.group().clone(),
    );

    // change current directory if necessary.
    if let Some(path) = path {
        let is_chdir = options.chdir().is_some();

        #[allow(unsafe_code)]
        unsafe {
            command.pre_exec(move || {
                let bytes = path.as_os_str().as_bytes();

                let c_path =
                    CString::new(bytes).expect("nul byte found in provided directory path");

                if let Err(err) = crate::system::chdir(&c_path) {
                    user_error!("unable to change directory to {}: {}", path.display(), err);
                    if is_chdir {
                        return Err(err);
                    }
                }

                Ok(())
            });
        }
    }

    if options.use_pty() {
        match UserTerm::open() {
            Ok(user_tty) => exec_pty(options.pid(), command, user_tty),
            Err(err) => {
                dev_info!("Could not open user's terminal, not allocating a pty: {err}");
                exec_no_pty(options.pid(), command)
            }
        }
    } else {
        exec_no_pty(options.pid(), command)
    }
}

/// Exit reason for the command executed by sudo.
#[derive(Debug)]
pub enum ExitReason {
    Code(i32),
    Signal(i32),
}

// Kill the process with increasing urgency.
//
// Based on `terminate_command`.
fn terminate_process(pid: ProcessId, use_killpg: bool) {
    let kill_fn = if use_killpg { killpg } else { kill };
    kill_fn(pid, SIGHUP).ok();
    kill_fn(pid, SIGTERM).ok();
    std::thread::sleep(Duration::from_secs(2));
    kill_fn(pid, SIGKILL).ok();
}

fn signal_fmt(signal: SignalNumber) -> Cow<'static, str> {
    signal_hook::low_level::signal_name(signal)
        .or_else(|| (signal == SIGCONT_FG).then_some("SIGCONT_FG"))
        .or_else(|| (signal == SIGCONT_BG).then_some("SIGCONT_BG"))
        .map(|name| name.into())
        .unwrap_or_else(|| format!("unknown signal #{}", signal).into())
}

const fn cond_fmt<'a>(cond: bool, true_s: &'a str, false_s: &'a str) -> &'a str {
    if cond {
        true_s
    } else {
        false_s
    }
}

const fn opt_fmt(cond: bool, s: &str) -> &str {
    cond_fmt(cond, s, "")
}
