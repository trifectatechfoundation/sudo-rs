use std::process::ExitStatus;

use crate::Result;

/// command builder
pub struct Command {
    args: Vec<String>,
    user: Option<String>,
    stdin: Option<String>,
}

impl Command {
    /// constructs a new `Command` for launching a program at path `program`
    pub fn new(program: impl AsRef<str>) -> Self {
        Self {
            args: vec![program.as_ref().to_string()],
            user: None,
            stdin: None,
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
    pub fn as_user(&mut self, username: impl AsRef<str>) -> &mut Self {
        self.user = Some(username.as_ref().to_string());
        self
    }

    /// input to feed into the program via stdin
    ///
    /// NOTE this overrides the last `stdin` call
    pub fn stdin(&mut self, input: impl AsRef<str>) -> &mut Self {
        self.stdin = Some(input.as_ref().to_string());
        self
    }

    pub(super) fn get_args(&self) -> &[String] {
        &self.args
    }

    pub(super) fn get_stdin(&self) -> Option<&str> {
        self.stdin.as_deref()
    }

    pub(crate) fn get_user(&self) -> Option<&str> {
        self.user.as_deref()
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
