use core::str;
use std::{
    fs,
    process::{Command, ExitStatus, Stdio},
};

use tempfile::NamedTempFile;

use crate::{helpers, Result};

const DEFAULT_COMMAND: &[&str] = &["sleep", "infinity"];

pub struct Container {
    id: String,
}

impl Container {
    pub fn new(image: &str) -> Result<Self> {
        let mut cmd = Command::new("docker");
        cmd.args(["run", "-d", "--rm", image]).args(DEFAULT_COMMAND);
        let id = helpers::stdout(&mut cmd)?;
        validate_docker_id(&id, &cmd)?;

        Ok(Container { id })
    }

    pub fn exec(&self, cmd: &[impl AsRef<str>], user: As) -> Result<ExecOutput> {
        helpers::run(&mut self.docker_cmd(cmd, user))
    }

    /// Returns `$cmd`'s stdout if it successfully exists
    pub fn stdout(&self, cmd: &[impl AsRef<str>], user: As) -> Result<String> {
        helpers::stdout(&mut self.docker_cmd(cmd, user))
    }

    fn docker_cmd(&self, cmd: &[impl AsRef<str>], user: As) -> Command {
        let mut docker_cmd = Command::new("docker");
        docker_cmd.arg("exec");
        if let Some(user) = user.as_string() {
            docker_cmd.arg("--user");
            docker_cmd.arg(user);
        }
        docker_cmd.arg(&self.id);
        for arg in cmd {
            docker_cmd.arg(arg.as_ref());
        }
        docker_cmd
    }

    pub fn cp(&self, path_in_container: &str, file_contents: &str) -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        fs::write(&mut temp_file, file_contents)?;

        let src_path = temp_file.path().display().to_string();
        let dest_path = format!("{}:{path_in_container}", self.id);

        helpers::stdout(Command::new("docker").args(["cp", "-q", &src_path, &dest_path]))?;

        Ok(())
    }
}

#[derive(Clone, Copy)]
pub enum As<'a> {
    Root,
    User { name: &'a str },
    // UserGroup { user: &'a str, group: &'a str },
}

impl<'a> As<'a> {
    fn as_string(&self) -> Option<String> {
        let s = match self {
            As::Root => return None,
            As::User { name } => name.to_string(),
        };
        Some(s)
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        // running this to completion would block the current thread for several seconds so just
        // fire and forget
        let _ = Command::new("docker")
            .args(["stop", &self.id])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

pub struct ExecOutput {
    pub status: ExitStatus,
    pub stderr: String,
    pub stdout: String,
}

fn validate_docker_id(id: &str, cmd: &Command) -> Result<()> {
    if id.chars().any(|c| !c.is_ascii_hexdigit()) {
        return Err(
            format!("`{cmd:?}` return what appears to be an invalid docker id: {id}").into(),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    const IMAGE: &str = "ubuntu:22.04";

    #[test]
    #[ignore = "slow"]
    fn eventually_removes_container_on_drop() -> Result<()> {
        let mut check_cmd = Command::new("docker");
        let docker = Container::new(IMAGE)?;
        check_cmd.args(["ps", "--all", "--quiet", "--filter"]);
        check_cmd.arg(format!("id={}", docker.id));

        let matches = helpers::stdout(&mut check_cmd)?;
        assert_eq!(1, matches.lines().count());
        drop(docker);

        // wait for a bit until `stop` and `--rm` have done their work
        thread::sleep(Duration::from_secs(15));

        let matches = helpers::stdout(&mut check_cmd)?;
        assert_eq!(0, matches.lines().count());

        Ok(())
    }

    #[test]
    fn exec_as_root_works() -> Result<()> {
        let docker = Container::new(IMAGE)?;
        let output = docker.exec(&["true"], As::Root)?;
        assert!(output.status.success());
        let output = docker.exec(&["false"], As::Root)?;
        assert_eq!(Some(1), output.status.code());
        Ok(())
    }

    #[test]
    fn exec_as_user_named_root_works() -> Result<()> {
        let docker = Container::new(IMAGE)?;
        let output = docker.exec(&["true"], As::User { name: "root" })?;
        assert!(output.status.success());
        Ok(())
    }

    #[test]
    fn exec_as_non_root_user_works() -> Result<()> {
        let docker = Container::new(IMAGE)?;
        let username = "ferris";
        let output = docker.exec(&["useradd", username], As::Root)?;
        assert!(output.status.success());
        let output = docker.exec(&["true"], As::User { name: username })?;
        assert!(output.status.success());
        Ok(())
    }

    #[test]
    fn cp_works() -> Result<()> {
        let docker = Container::new(IMAGE)?;
        let expected = "Hello, world!";
        docker.cp("/tmp/file", expected)?;
        let actual = docker.stdout(&["cat", "/tmp/file"], As::Root)?;
        assert_eq!(expected, actual);
        Ok(())
    }
}
