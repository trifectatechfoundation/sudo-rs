#![forbid(unsafe_code)]
use std::{
    ffi::c_int,
    io,
    os::unix::process::CommandExt,
    process::{Command, ExitStatus},
    time::Duration,
};

use signal_hook::{
    consts::*,
    iterator::{exfiltrator::WithOrigin, SignalsInfo},
    low_level::siginfo::{Cause, Origin, Process, Sent},
};
use sudo_common::context::{Context, Environment};
use sudo_system::{getpgid, kill};

/// We only handle the signals that ogsudo handles.
const SIGNALS: &[c_int] = &[
    SIGINT, SIGQUIT, SIGTSTP, SIGTERM, SIGHUP, SIGALRM, SIGPIPE, SIGUSR1, SIGUSR2, SIGCHLD,
    SIGCONT, SIGWINCH,
];

/// Based on `ogsudo`s `exec_nopty` function.
pub fn run_command(ctx: Context<'_>, env: Environment) -> io::Result<ExitStatus> {
    // FIXME: should we pipe the stdio streams?
    let mut cmd = Command::new(ctx.command.command)
        .args(ctx.command.arguments)
        .uid(ctx.target_user.uid)
        .gid(ctx.target_user.gid)
        .env_clear()
        .envs(env)
        .spawn()?;

    let cmd_pid = cmd.id() as i32;

    let mut signals = SignalsInfo::<WithOrigin>::new(SIGNALS)?;

    loop {
        // First we check for the command
        if let Some(code) = cmd.try_wait()? {
            return Ok(code);
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
                    if !user_signaled || is_self_terminating(info.process, cmd_pid, ctx.pid) {
                        continue;
                    }
                }
                _ => {
                    // Skip the signal if it was sent by the user and it is self-terminating.
                    if user_signaled && is_self_terminating(info.process, cmd_pid, ctx.pid) {
                        continue;
                    }
                }
            }

            let status = if info.signal == SIGALRM {
                // Kill the command with increasing urgency.
                // Based on `terminate_command`.
                kill(cmd_pid, SIGHUP);
                kill(cmd_pid, SIGTERM);
                std::thread::sleep(Duration::from_secs(2));
                kill(cmd_pid, SIGKILL)
            } else {
                kill(cmd_pid, info.signal)
            };

            if status != 0 {
                eprintln!("kill failed");
            }
        }
    }
}

/// Decides if the signal sent by `process` is self-terminating.
///
/// A signal is self-terminating if the PID of the `process`:
/// - is the same PID of the command, or
/// - is in the process group of the command and either sudo of the command are the leader.
fn is_self_terminating(process: Option<Process>, cmd_pid: i32, sudo_pid: i32) -> bool {
    if let Some(process) = process {
        if process.pid != 0 {
            if process.pid == cmd_pid {
                return true;
            }
            let grp_leader = getpgid(process.pid);

            if grp_leader != -1 || grp_leader == cmd_pid || grp_leader == sudo_pid {
                return true;
            }
        }
    }

    false
}
