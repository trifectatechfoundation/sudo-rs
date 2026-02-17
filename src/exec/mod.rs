mod event;
mod io_util;
mod no_pty;
#[cfg(target_os = "linux")]
mod noexec;
mod use_pty;

use std::{
    borrow::Cow,
    convert::Infallible,
    env,
    ffi::{OsStr, OsString, c_int},
    io,
    os::unix::{ffi::OsStrExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{self, Command},
    time::Duration,
};

use crate::{
    common::{
        HARDENED_ENUM_VALUE_0, HARDENED_ENUM_VALUE_1, HARDENED_ENUM_VALUE_2, bin_serde::BinPipe,
    },
    exec::no_pty::exec_no_pty,
    log::{dev_info, dev_warn, user_error},
    system::{
        _exit, ForkResult, Group, User, fork,
        interface::ProcessId,
        kill, killpg, mark_fds_as_cloexec, set_target_user, setpgid,
        signal::{SignalNumber, SignalSet, SignalsState, consts::*, exit_with_signal, signal_name},
        term::UserTerm,
        wait::{Wait, WaitError, WaitOptions},
    },
};

use self::{
    event::{EventRegistry, Process},
    io_util::was_interrupted,
    use_pty::{SIGCONT_BG, SIGCONT_FG, exec_pty},
};

#[cfg(target_os = "linux")]
use self::noexec::SpawnNoexecHandler;
#[cfg(not(target_os = "linux"))]
enum SpawnNoexecHandler {}
#[cfg(not(target_os = "linux"))]
impl SpawnNoexecHandler {
    fn spawn(self) {}
}

#[derive(Debug, Copy, Clone)]
#[cfg_attr(test, derive(PartialEq))]
#[repr(u32)]
pub enum Umask {
    /// Keep the umask of the parent process.
    Preserve = HARDENED_ENUM_VALUE_0,
    /// Mask out more of the permission bits in the new umask.
    Extend(libc::mode_t) = HARDENED_ENUM_VALUE_1,
    /// Override the umask of the parent process entirely with the given umask.
    Override(libc::mode_t) = HARDENED_ENUM_VALUE_2,
}

pub struct RunOptions<'a> {
    pub command: &'a Path,
    pub arguments: &'a [OsString],
    pub arg0: Option<&'a Path>,
    pub chdir: Option<PathBuf>,
    pub is_login: bool,
    pub user: &'a User,
    pub group: &'a Group,
    pub umask: Umask,

    pub background: bool,
    pub use_pty: bool,
    pub noexec: bool,
}

/// Based on `ogsudo`s `exec_pty` function.
///
/// Returns the [`ExitReason`] of the command and a function that restores the default handler for
/// signals once its called.
pub fn run_command(
    options: RunOptions<'_>,
    env: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
) -> io::Result<ExitReason> {
    if options.background {
        // SAFETY: There should be no other threads at this point.
        match unsafe { fork() }? {
            ForkResult::Parent(_) => process::exit(0),
            ForkResult::Child => {
                // Child continues in an orphaned process group.
                // Reads from the terminal fail with EIO.
                // Writes succeed unless tostop is set on the terminal.
                setpgid(ProcessId::new(0), ProcessId::new(0))?;
            }
        }
    }

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

    let spawn_noexec_handler = if options.noexec {
        #[cfg(not(target_os = "linux"))]
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "NOEXEC is currently only supported on Linux",
        ));

        #[cfg(target_os = "linux")]
        Some(noexec::add_noexec_filter(&mut command)?)
    } else {
        None
    };

    // Decide if the pwd should be changed. `--chdir` takes precedence over `-i`.
    let path = options
        .chdir
        .as_ref()
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
                    user_error!(
                        "unable to change directory to {path}: {error}",
                        path = path.display(),
                        error = err
                    );
                    if is_chdir {
                        return Err(err);
                    }
                }

                Ok(())
            });
        }
    }

    // SAFETY: Umask is async-signal-safe.
    unsafe {
        let umask = options.umask;

        command.pre_exec(move || {
            match umask {
                Umask::Preserve => {}
                Umask::Extend(umask) => {
                    // The only options to get the existing umask are overwriting it or
                    // parsing a /proc file. Given that this is a single-threaded context,
                    // overwrite it with a safe value is fine and the simpler option.
                    let existing_umask = libc::umask(0o777);
                    libc::umask(existing_umask | umask);
                }
                Umask::Override(umask) => {
                    libc::umask(umask);
                }
            }

            Ok(())
        });
    }

    let sudo_pid = ProcessId::new(std::process::id() as i32);

    if options.use_pty {
        match UserTerm::open() {
            Ok(user_tty) => exec_pty(
                sudo_pid,
                spawn_noexec_handler,
                command,
                user_tty,
                options.user,
                options.background,
            ),
            Err(err) => {
                dev_info!("Could not open user's terminal, not allocating a pty: {err}");
                exec_no_pty(sudo_pid, spawn_noexec_handler, command)
            }
        }
    } else {
        exec_no_pty(sudo_pid, spawn_noexec_handler, command)
    }
}

/// Exit reason for the command executed by sudo.
#[derive(Debug)]
pub enum ExitReason {
    Code(i32),
    Signal(i32),
}

impl ExitReason {
    pub(crate) fn exit_process(self) -> Result<Infallible, crate::common::Error> {
        match self {
            ExitReason::Code(code) => process::exit(code),
            ExitReason::Signal(signal) => exit_with_signal(signal),
        }
    }
}

fn exec_command(
    mut command: Command,
    original_set: Option<SignalSet>,
    mut original_signal: SignalsState,
    mut errpipe_tx: BinPipe<i32>,
) -> ! {
    // Restore the signal handlers of modified signals
    if let Err(err) = original_signal.restore() {
        dev_warn!("cannot restore signal states: {err}");
    }

    // Restore the signal mask now that the handlers have been setup.
    if let Some(set) = original_set {
        if let Err(err) = set.set_mask() {
            dev_warn!("cannot restore signal mask: {err}");
        }
    }

    if let Err(err) = mark_fds_as_cloexec() {
        dev_warn!("failed to close the universe: {err}");
        // Send the error to the monitor using the pipe.
        if let Some(error_code) = err.raw_os_error() {
            errpipe_tx.write(&error_code).ok();
        }

        // We call `_exit` instead of `exit` to avoid flushing the parent's IO streams by accident.
        _exit(1);
    }

    let err = command.exec();

    dev_warn!("failed to execute command: {err}");
    // If `exec` returns, it means that executing the command failed. Send the error to the
    // monitor using the pipe.
    if let Some(error_code) = err.raw_os_error() {
        errpipe_tx.write(&error_code).ok();
    }

    // We call `_exit` instead of `exit` to avoid flushing the parent's IO streams by accident.
    _exit(1);
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
    if cond { true_s } else { false_s }
}

const fn opt_fmt(cond: bool, s: &str) -> &str {
    cond_fmt(cond, s, "")
}
