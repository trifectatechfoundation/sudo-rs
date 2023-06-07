#![deny(unsafe_code)]

mod monitor;
mod pty;
mod signal;

use std::{
    ffi::{CString, OsStr},
    io,
    mem::size_of,
    os::unix::{ffi::OsStrExt, process::ExitStatusExt},
    os::{fd::OwnedFd, unix::process::CommandExt},
    process::{Command, ExitStatus},
};

use sudo_common::{context::LaunchType::Login, Context, Environment};
use sudo_log::user_error;
use sudo_system::{fork, openpty, pipe, read, set_target_user, write};

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

    let (pty_leader, pty_follower) = openpty()?;
    let (rx, tx) = pipe()?;
    let monitor_pid = fork()?;
    // Monitor logic. Based on `exec_monitor`.
    if monitor_pid == 0 {
        match monitor::MonitorRelay::new(command, pty_follower, tx)?.run()? {}
    } else {
        pty::PtyRelay::new(monitor_pid, ctx.process.pid, pty_leader, rx)?.run()
    }
}

/// Exit reason for the command executed by sudo.
pub enum ExitReason {
    Code(i32),
    Signal(i32),
}

impl ExitReason {
    fn send(self, tx: &OwnedFd) -> io::Result<()> {
        let mut bytes = [0u8; size_of::<u8>() + size_of::<i32>()];
        let (prefix_bytes, int_bytes) = bytes.split_at_mut(size_of::<u8>());
        match self {
            Self::Code(code) => {
                int_bytes.copy_from_slice(&code.to_ne_bytes());
            }
            Self::Signal(signal) => {
                prefix_bytes.copy_from_slice(&1u8.to_ne_bytes());
                int_bytes.copy_from_slice(&signal.to_ne_bytes());
            }
        }

        write(tx, &bytes)?;

        Ok(())
    }

    fn recv(rx: &OwnedFd) -> io::Result<Self> {
        let mut bytes = [0u8; size_of::<u8>() + size_of::<i32>()];

        read(rx, &mut bytes)?;

        let (prefix_bytes, int_bytes) = {
            let (hd, tl) = bytes.split_at(size_of::<u8>());
            (hd.try_into().unwrap(), tl.try_into().unwrap())
        };

        let prefix = u8::from_ne_bytes(prefix_bytes);
        let int = i32::from_ne_bytes(int_bytes);
        if prefix == 0 {
            Ok(Self::Code(int))
        } else {
            Ok(Self::Signal(int))
        }
    }

    fn from_status(status: ExitStatus) -> Self {
        if let Some(code) = status.code() {
            Self::Code(code)
        } else {
            Self::Signal(status.signal().unwrap())
        }
    }
}
