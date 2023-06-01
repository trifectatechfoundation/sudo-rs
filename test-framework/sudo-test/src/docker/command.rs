use core::fmt;
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
    pub fn wait(self) -> Result<Output> {
        let output = self.inner.wait_with_output()?;
        output.try_into()
    }

    /// attempts to collect the exit status of the child if it has already exited.
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        Ok(self.inner.try_wait()?)
    }
}

/// the output of a finished `Command`
#[must_use]
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
    ///
    /// if it didn't the error value will include the exit code and the program's stderr
    pub fn assert_success(&self) -> Result<()> {
        if self.status.success() {
            Ok(())
        } else {
            let exit_code = if let Some(code) = self.status.code() {
                format!("exit code {}", code)
            } else {
                "a non-zero exit code".to_string()
            };

            Err(format!("program failed with {exit_code}. stderr:\n{}", self.stderr).into())
        }
    }

    /// the collected standard output of the finished `Command`
    ///
    /// NOTE this method implicitly runs `assert_success` before granting access to `stdout`
    pub fn stdout(self) -> Result<String> {
        self.assert_success()?;
        Ok(self.stdout)
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
