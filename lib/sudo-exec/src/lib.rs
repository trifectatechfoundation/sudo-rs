#![deny(unsafe_code)]

use std::{
    ffi::{c_int, CString, OsStr},
    io,
    os::unix::ffi::OsStrExt,
    os::unix::process::{CommandExt, ExitStatusExt},
    process::{Command, ExitStatus},
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
use sudo_system::{getpgid, kill, set_target_user};

/// We only handle the signals that ogsudo handles.
const SIGNALS: &[c_int] = &[
    SIGINT, SIGQUIT, SIGTSTP, SIGTERM, SIGHUP, SIGALRM, SIGPIPE, SIGUSR1, SIGUSR2, SIGCHLD,
    SIGCONT, SIGWINCH,
];

/// Based on `ogsudo`s `exec_nopty` function.
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
    // spawn and exec to command
    let mut child = command.spawn()?;

    let child_pid = child.id() as i32;

    let mut signals = SignalsInfo::<WithOrigin>::new(SIGNALS)?;

    loop {
        // First we check if the child is finished
        if let Some(status) = child.try_wait()? {
            if let Some(signal) = status.signal() {
                // If the child terminated because of a signal, we send this signal to sudo
                // itself to match the original sudo behavior. If we fail we just return the status
                // code.
                if kill(ctx.process.pid, signal) != -1 {
                    // Given that we overwrote the default handlers for all the signals, we must
                    // emulate them to handle the signal we just sent correctly.
                    for info in signals.pending() {
                        emulate_default_handler(info.signal)?;
                    }
                }
            }

            return Ok(status);
        }

        // Then we check any pending signals that we received.
        for info in signals.pending() {
            let user_signaled = info.cause == Cause::Sent(Sent::User);
            match info.signal {
                SIGCHLD => {
                    // FIXME: check `handle_sigchld_nopty`
                    // We just wait until all the children are done.
                    continue;
                }
                SIGWINCH | SIGINT | SIGQUIT | SIGTSTP => {
                    // Skip the signal if it was not sent by the user or if it is self-terminating.
                    if !user_signaled
                        || is_self_terminating(info.process, child_pid, ctx.process.pid)
                    {
                        continue;
                    }
                }
                _ => {
                    // Skip the signal if it was sent by the user and it is self-terminating.
                    if user_signaled
                        && is_self_terminating(info.process, child_pid, ctx.process.pid)
                    {
                        continue;
                    }
                }
            }

            let status = if info.signal == SIGALRM {
                // Kill the child with increasing urgency.
                // Based on `terminate_command`.
                kill(child_pid, SIGHUP);
                kill(child_pid, SIGTERM);
                std::thread::sleep(Duration::from_secs(2));
                kill(child_pid, SIGKILL)
            } else {
                kill(child_pid, info.signal)
            };

            if status != 0 {
                user_error!("kill failed");
            }
        }
    }
}

/// Decides if the signal sent by `process` is self-terminating.
///
/// A signal is self-terminating if the PID of the `process`:
/// - is the same PID of the child, or
/// - is in the process group of the child and either sudo or the child is the leader.
fn is_self_terminating(process: Option<Process>, child_pid: i32, sudo_pid: i32) -> bool {
    if let Some(process) = process {
        if process.pid != 0 {
            if process.pid == child_pid {
                return true;
            }
            let grp_leader = getpgid(process.pid);

            if grp_leader != -1 {
                if grp_leader == child_pid || grp_leader == sudo_pid {
                    return true;
                }
            } else {
                auth_warn!("Could not fetch process group ID");
            }
        }
    }

    false
}
