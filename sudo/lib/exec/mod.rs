#![deny(unsafe_code)]

mod backchannel;
mod event;
mod io_util;
mod monitor;
mod parent;

use std::{
    ffi::{CString, OsStr},
    io,
    os::unix::ffi::OsStrExt,
    os::unix::process::CommandExt,
    process::{exit, Command},
};

use crate::common::{context::LaunchType::Login, Context, Environment};
use crate::log::user_error;
use crate::system::{fork, set_target_user, term::openpty};
use backchannel::BackchannelPair;
use parent::ParentClosure;

/// Based on `ogsudo`s `exec_pty` function.
///
/// Returns the [`ExitReason`] of the command and a function that restores the default handler for
/// signals once its called.
pub fn run_command(ctx: Context, env: Environment) -> io::Result<(ExitReason, impl FnOnce())> {
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

                if let Err(err) = crate::system::chdir(&c_path) {
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

    let (pty_leader, pty_follower) = openpty()?;

    let mut backchannels = BackchannelPair::new()?;

    // FIXME: We should block all the incoming signals before forking and unblock them just after
    // initializing the signal handlers.
    let monitor_pid = fork()?;
    // Monitor logic. Based on `exec_monitor`.
    if monitor_pid == 0 {
        // If `exec_monitor` returns, it means we failed to execute the command somehow.
        if let Err(err) = monitor::exec_monitor(pty_follower, command, &mut backchannels.monitor) {
            backchannels.monitor.send(&err.into()).ok();
            exit(1)
        }
    }

    let (parent, mut dispatcher) = ParentClosure::new(
        monitor_pid,
        ctx.process.pid,
        pty_leader,
        backchannels.parent,
    )?;
    parent
        .run(&mut dispatcher)
        .map(|exit_reason| (exit_reason, move || drop(dispatcher)))
}

/// Exit reason for the command executed by sudo.
#[derive(Debug)]
pub enum ExitReason {
    Code(i32),
    Signal(i32),
}
