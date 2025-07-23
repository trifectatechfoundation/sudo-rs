use core::fmt;
use std::os::unix::process::ExitStatusExt;
use std::process::{self, ExitStatus};

use crate::{Error, Result};

/// command builder
pub struct Command {
    args: Vec<String>,
    as_: Option<As>,
    stdin: Option<String>,
    tty: bool,
}

pub enum As {
    User(String),
    UserId(u16),
}

impl fmt::Display for As {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            As::User(name) => f.write_str(name),
            As::UserId(id) => write!(f, "{id}"),
        }
    }
}

impl Command {
    /// constructs a new `Command` for launching a program at path `program`
    pub fn new(program: impl AsRef<str>) -> Self {
        Self {
            args: vec![program.as_ref().to_string()],
            as_: None,
            stdin: None,
            tty: false,
        }
    }

    /// adds an argument to pass to the program
    pub fn arg(&mut self, arg: impl AsRef<str>) -> &mut Self {
        self.args.push(arg.as_ref().to_string());
        self
    }

    /// adds multiple arguments to pass to the program
    pub fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<str>>) -> &mut Self {
        args.into_iter().for_each(|arg| {
            self.arg(arg);
        });
        self
    }

    /// the user to run the program as
    ///
    /// NOTE if this method is not used the default is to run the program as `root`
    ///
    /// # Panics
    ///
    /// - if both `as_user` and `as_user_id` are specified
    pub fn as_user(&mut self, username: impl AsRef<str>) -> &mut Self {
        assert!(self.get_as().is_none());
        self.as_ = Some(As::User(username.as_ref().to_string()));
        self
    }

    /// the user ID to run the program as
    ///
    /// NOTE if this method is not used the default is to run the program as `root`
    pub fn as_user_id(&mut self, user_id: u16) -> &mut Self {
        assert!(self.get_as().is_none());
        self.as_ = Some(As::UserId(user_id));
        self
    }

    /// input to feed into the program via stdin
    ///
    /// NOTE this overrides the last `stdin` call
    pub fn stdin(&mut self, input: impl AsRef<str>) -> &mut Self {
        self.stdin = Some(input.as_ref().to_string());
        self
    }

    /// whether to allocate a pseudo-TTY for the execution of this command
    ///
    /// equivalent to docker's `--tty` flag
    pub fn tty(&mut self, tty: bool) -> &mut Self {
        self.tty = tty;
        self
    }

    pub(super) fn get_args(&self) -> &[String] {
        &self.args
    }

    pub(super) fn get_stdin(&self) -> Option<&str> {
        self.stdin.as_deref()
    }

    pub(crate) fn get_as(&self) -> Option<&As> {
        self.as_.as_ref()
    }

    pub(crate) fn get_tty(&self) -> bool {
        self.tty
    }
}

/// A process spawned in the test environment
pub struct Child {
    inner: process::Child,
}

impl Child {
    pub(super) fn new(inner: process::Child) -> Self {
        Self { inner }
    }

    /// waits for the child to exit and collects its stdout and stderr
    #[track_caller]
    pub fn wait(self) -> Output {
        let res = (|| -> Result<Output> {
            let output = self.inner.wait_with_output()?;
            output.try_into()
        })();
        match res {
            Ok(output) => output,
            Err(err) => panic!("waiting for child failed: {err}"),
        }
    }

    /// attempts to collect the exit status of the child if it has already exited.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        Ok(self.inner.try_wait()?)
    }

    /// Send SIGKILL to the process.
    pub fn kill(&mut self) -> Result<()> {
        Ok(self.inner.kill()?)
    }
}

/// the output of a finished `Command`
#[must_use]
#[derive(Debug)]
pub struct Output {
    pub(super) status: ExitStatus,
    pub(super) stderr: String,
    pub(super) stdout: String,
}

impl Output {
    /// the status (exit code) of the finished `Command`
    pub fn status(&self) -> ExitStatus {
        self.status
    }

    /// the collected standard error of the finished `Command`
    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    /// helper method that asserts that the program exited successfully
    #[track_caller]
    pub fn assert_success(&self) {
        if !self.status.success() {
            panic!(
                "program failed with {}\nstdout:\n{}\n\nstderr:\n{}",
                self.status, self.stdout, self.stderr
            );
        }
    }

    /// helper method that asserts that the program exited with the given exit code
    #[track_caller]
    pub fn assert_exit_code(&self, code: i32) {
        assert_ne!(code, 0, "use assert_success to check for success");
        if self.status.code() != Some(code) {
            panic!(
                "program failed with {}, expected exit code {code}\nstdout:\n{}\n\nstderr:\n{}",
                self.status, self.stdout, self.stderr
            );
        }
    }

    /// helper method that asserts that the program got killed by the given signal
    #[track_caller]
    pub fn assert_signal(&self, signal: i32) {
        assert_ne!(signal, 0, "0 is not a valid signal");
        if self.status.signal() != Some(signal) {
            panic!(
                "program failed with {}, expected signal {signal}\nstdout:\n{}\n\nstderr:\n{}",
                self.status, self.stdout, self.stderr
            );
        }
    }

    /// the collected standard output of the finished `Command`
    ///
    /// NOTE this method implicitly runs `assert_success` before granting access to `stdout`
    #[track_caller]
    pub fn stdout(self) -> String {
        self.assert_success();
        self.stdout
    }

    /// like `stdout` but does not check the exit code
    pub fn stdout_unchecked(&self) -> &str {
        &self.stdout
    }
}

impl TryFrom<process::Output> for Output {
    type Error = Error;

    fn try_from(output: process::Output) -> std::result::Result<Self, Self::Error> {
        let mut stderr = String::from_utf8(output.stderr)?;
        let mut stdout = String::from_utf8(output.stdout)?;

        // it's a common pitfall to forget to remove the trailing '\n' so remove it here
        if stderr.ends_with('\n') {
            stderr.pop();
        }

        if stdout.ends_with('\n') {
            stdout.pop();
        }

        Ok(Output {
            status: output.status,
            stderr,
            stdout,
        })
    }
}
