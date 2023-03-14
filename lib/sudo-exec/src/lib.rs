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
    low_level::siginfo::{Cause, Origin, Sent},
};
use sudo_common::context::{Context, Environment};
use sudo_system::{getpgid, kill};

/// We do not handle `SIGKILL`, `SIGSTOP`, `SIGILL`, `SIGFPE` nor `SIGSEGV` because those should
/// not be intercepted and replaced. according to `POSIX`.
// FIXME: are we missing any signals? `SIGEMT`, `SIGLOST` and `SIGPWR` are not exposed by
// `signal-hook`.
const SIGNALS: &[c_int] = &[
    SIGABRT, SIGALRM, SIGBUS, SIGCHLD, SIGCONT, SIGHUP, SIGINT, SIGPIPE, SIGPROF, SIGQUIT, SIGSYS,
    SIGTERM, SIGTRAP, SIGTSTP, SIGTTIN, SIGTTOU, SIGURG, SIGUSR1, SIGUSR2, SIGVTALRM, SIGWINCH,
    SIGXCPU, SIGXFSZ, SIGIO,
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
        for Origin {
            signal,
            process,
            cause,
            ..
        } in signals.pending()
        {
            match signal {
                SIGCHLD => {
                    // FIXME: check `handle_sigchld_nopty`
                    cmd.kill()?;
                }
                SIGWINCH | SIGINT | SIGQUIT | SIGTSTP => {
                    if cause != Cause::Sent(Sent::User) {
                        continue;
                    }

                    if let Some(process) = process {
                        if process.pid != 0 {
                            if process.pid == cmd_pid {
                                continue;
                            }
                            let process_grp = getpgid(process.pid);
                            if process_grp != -1 {
                                // FIXME: we should also check that the process group is not the
                                // sudo PID.
                                if process_grp == cmd_pid {
                                    continue;
                                }
                            }
                        }
                    }
                }
                _ => {
                    if cause == Cause::Sent(Sent::User) {
                        if let Some(process) = process {
                            if process.pid != 0 {
                                if process.pid == cmd_pid {
                                    continue;
                                }
                                let process_grp = getpgid(process.pid);
                                if process_grp != -1 {
                                    // FIXME: we should also check that the process group is not the
                                    // sudo PID.
                                    if process_grp == cmd_pid {
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let status = if signal == SIGALRM {
                kill(cmd_pid, SIGHUP);
                kill(cmd_pid, SIGTERM);
                std::thread::sleep(Duration::from_secs(2));
                kill(cmd_pid, SIGKILL)
            } else {
                kill(cmd_pid, signal)
            };

            if status != 0 {
                eprintln!("kill failed");
            }
        }

        if let Some(code) = cmd.try_wait()? {
            return Ok(code);
        }
    }
}
