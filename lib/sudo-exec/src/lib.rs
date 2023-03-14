use std::{
    ffi::c_int,
    io,
    os::unix::process::CommandExt,
    process::{Command, ExitStatus, Stdio},
};

use libc::getpgid;
use signal_hook::{
    consts::*,
    iterator::{exfiltrator::WithOrigin, SignalsInfo},
    low_level::siginfo::{Cause, Origin, Sent},
};
use sudo_common::context::{Context, Environment};

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
    let mut cmd = Command::new(ctx.command.command)
        .args(ctx.command.arguments)
        .uid(ctx.target_user.uid)
        .gid(ctx.target_user.gid)
        .env_clear()
        .envs(env)
        // FIXME: should we pipe everything?
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
                SIGWINCH => {
                    // FIXME: check `handle_sigwinch`
                    cmd.kill()?;
                }
                SIGINT | SIGQUIT | SIGTSTP => {
                    if cause != Cause::Sent(Sent::User) {
                        continue;
                    }

                    if let Some(process) = process {
                        if process.pid != 0 {
                            if process.pid == cmd_pid {
                                continue;
                            }
                            let process_grp = unsafe { getpgid(process.pid) };
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
                                let process_grp = unsafe { getpgid(process.pid) };
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

            if signal == SIGALRM {
                // FIXME: check `terminate_command` to match behavior.
                cmd.kill()?;
            } else if unsafe { libc::kill(cmd_pid, signal) } != 0 {
                eprintln!("kill failed");
            }
        }

        if let Some(code) = cmd.try_wait()? {
            return Ok(code);
        }
    }
}
