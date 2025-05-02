mod event;
mod io_util;
mod no_pty;
mod use_pty;

use std::{
    borrow::Cow,
    env,
    ffi::{c_int, OsStr},
    io,
    os::unix::ffi::OsStrExt,
    os::unix::process::CommandExt,
    path::Path,
    process::Command,
    time::Duration,
};

use crate::{
    exec::no_pty::exec_no_pty,
    log::{dev_info, dev_warn, user_error},
    system::{
        interface::ProcessId,
        killpg,
        signal::{consts::*, signal_name},
        wait::{Wait, WaitError, WaitOptions},
    },
    system::{kill, set_target_user, signal::SignalNumber, term::UserTerm, Group, User},
};

use self::{
    event::{EventRegistry, Process},
    io_util::was_interrupted,
    use_pty::{exec_pty, SIGCONT_BG, SIGCONT_FG},
};

pub struct RunOptions<'a> {
    pub command: &'a Path,
    pub arguments: &'a [String],
    pub arg0: Option<&'a Path>,
    pub chdir: Option<&'a Path>,
    pub is_login: bool,
    pub user: &'a User,
    pub group: &'a Group,

    pub use_pty: bool,
}

/// Based on `ogsudo`s `exec_pty` function.
///
/// Returns the [`ExitReason`] of the command and a function that restores the default handler for
/// signals once its called.
pub fn run_command(
    options: RunOptions<'_>,
    env: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
) -> io::Result<ExitReason> {
    // FIXME: should we pipe the stdio streams?
    let qualified_path = options.command;
    let mut command = Command::new(qualified_path);
    // reset env and set filtered environment
    command.args(options.arguments).env_clear().envs(env);
    // set the arg0 to the requested string
    // TODO: this mechanism could perhaps also be used to set the arg0 for login shells, as below
    if let Some(arg0) = options.arg0 {
        command.arg0(arg0);
    }

    if options.is_login {
        // signal to the operating system that the command is a login shell by prefixing "-"
        let mut process_name = qualified_path
            .file_name()
            .map(|osstr| osstr.as_bytes().to_vec())
            .unwrap_or_default();
        process_name.insert(0, b'-');
        command.arg0(OsStr::from_bytes(&process_name));
    }

    // Decide if the pwd should be changed. `--chdir` takes precedence over `-i`.
    let path = options
        .chdir
        .map(|chdir| chdir.to_owned())
        .or_else(|| options.is_login.then(|| options.user.home.clone().into()))
        .clone();

    // set target user and groups
    set_target_user(&mut command, options.user.clone(), options.group.clone());

    // change current directory if necessary.
    if let Some(path) = path {
        let is_chdir = options.chdir.is_some();

        // SAFETY: Chdir as used internally by set_current_dir is async-signal-safe. The logger we
        // use is also async-signal-safe.
        unsafe {
            command.pre_exec(move || {
                if let Err(err) = env::set_current_dir(&path) {
                    user_error!("unable to change directory to {}: {}", path.display(), err);
                    if is_chdir {
                        return Err(err);
                    }
                }

                Ok(())
            });
        }
    }

    let sudo_pid = ProcessId::new(std::process::id() as i32);

    if options.use_pty {
        match UserTerm::open() {
            Ok(user_tty) => exec_pty(sudo_pid, command, user_tty),
            Err(err) => {
                dev_info!("Could not open user's terminal, not allocating a pty: {err}");
                exec_no_pty(sudo_pid, command)
            }
        }
    } else {
        exec_no_pty(sudo_pid, command)
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

trait HandleSigchld: Process {
    const OPTIONS: WaitOptions;

    fn on_exit(&mut self, exit_code: c_int, registry: &mut EventRegistry<Self>);
    fn on_term(&mut self, signal: SignalNumber, registry: &mut EventRegistry<Self>);
    fn on_stop(&mut self, signal: SignalNumber, registry: &mut EventRegistry<Self>);
}

fn handle_sigchld<T: HandleSigchld>(
    handler: &mut T,
    registry: &mut EventRegistry<T>,
    child_name: &'static str,
    child_pid: ProcessId,
) {
    let status = loop {
        match child_pid.wait(T::OPTIONS) {
            Err(WaitError::Io(err)) if was_interrupted(&err) => {}
            // This only happens if we receive `SIGCHLD` but there's no status update from the
            // monitor.
            Err(WaitError::Io(err)) => {
                return dev_info!("cannot wait for {child_pid} ({child_name}): {err}");
            }
            // This only happens if the monitor exited and any process already waited for the
            // monitor.
            Err(WaitError::NotReady) => {
                return dev_info!("{child_pid} ({child_name}) has no status report");
            }
            Ok((_pid, status)) => break status,
        }
    };
    if let Some(exit_code) = status.exit_status() {
        dev_info!("{child_pid} ({child_name}) exited with status code {exit_code}");
        handler.on_exit(exit_code, registry)
    } else if let Some(signal) = status.stop_signal() {
        dev_info!(
            "{child_pid} ({child_name}) was stopped by {}",
            signal_fmt(signal),
        );
        handler.on_stop(signal, registry)
    } else if let Some(signal) = status.term_signal() {
        dev_info!(
            "{child_pid} ({child_name}) was terminated by {}",
            signal_fmt(signal),
        );
        handler.on_term(signal, registry)
    } else if status.did_continue() {
        dev_info!("{child_pid} ({child_name}) continued execution");
    } else {
        dev_warn!("unexpected wait status for {child_pid} ({child_name})")
    }
}

fn signal_fmt(signal: SignalNumber) -> Cow<'static, str> {
    match signal_name(signal) {
        name @ Cow::Owned(_) => match signal {
            SIGCONT_BG => "SIGCONT_BG".into(),
            SIGCONT_FG => "SIGCONT_FG".into(),
            _ => name,
        },
        name => name,
    }
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
