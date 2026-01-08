use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::{io, process};

use libc::O_CLOEXEC;

use crate::cutils::cerr;
use crate::log::user_error;
use crate::system::interface::ProcessId;
use crate::system::{ForkResult, audit, fork, mark_fds_as_cloexec};

pub(super) fn spawn_askpass(program: &Path, prompt: &str) -> io::Result<(ProcessId, OwnedFd)> {
    // Create socket
    let mut pipes = [-1, -1];
    // SAFETY: A valid pointer to a mutable array of 2 fds is passed in.
    unsafe {
        #[cfg(not(target_os = "macos"))]
        cerr(libc::pipe2(pipes.as_mut_ptr()), O_CLOEXEC)?;
        #[cfg(target_os = "macos")]
        cerr(crate::cutils::pipe2(pipes.as_mut_ptr(), O_CLOEXEC))?;
    }

    // SAFETY: pipe2 created two owned pipe fds
    let (pipe_read, pipe_write) = unsafe {
        (
            OwnedFd::from_raw_fd(pipes[0]),
            OwnedFd::from_raw_fd(pipes[1]),
        )
    };

    // Spawn child
    // SAFETY: There should be no other threads at this point.
    let ForkResult::Parent(command_pid) = unsafe { fork() }.unwrap() else {
        drop(pipe_read);
        handle_child(program, prompt, pipe_write)
    };
    drop(pipe_write);

    Ok((command_pid, pipe_read))
}

fn handle_child(program: &Path, prompt: &str, stdout: OwnedFd) -> ! {
    if let Err(e) = mark_fds_as_cloexec() {
        eprintln_ignore_io_error!("Failed to mark fds as CLOEXEC: {e}");
        process::exit(1);
    };

    // root privileges are dangerous after this point, since we are about to
    // execute a command under control of the user, so drop them
    audit::irrevocably_drop_privileges();

    // Exec askpass program
    let error = Command::new(program).arg(prompt).stdout(stdout).exec();
    user_error!(
        "Failed to run askpass program {path}: {error}",
        path = program.display(),
        error = error
    );
    process::exit(1);
}
