use core::str;
use std::{
    env,
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    process::{self, Command as StdCommand, Stdio},
};

use tempfile::NamedTempFile;

use crate::{base_image, Result, SudoUnderTest};

pub use self::command::{As, Child, Command, Output};

mod command;

const DOCKER_RUN_COMMAND: &[&str] = &["sleep", "infinity"];

pub struct Container {
    id: String,
}

impl Container {
    #[cfg(test)]
    fn new(image: &str) -> Result<Self> {
        Self::new_with_hostname(image, None)
    }

    pub fn new_with_hostname(image: &str, hostname: Option<&str>) -> Result<Self> {
        let mut docker_run = StdCommand::new("docker");
        docker_run.args(["run", "--detach"]);
        if let Some(hostname) = hostname {
            docker_run.args(["--hostname", hostname]);
        }
        docker_run.args(["--rm", image]).args(DOCKER_RUN_COMMAND);
        let id = run(&mut docker_run, None)?.stdout()?;
        validate_docker_id(&id, &docker_run)?;

        Ok(Container { id })
    }

    pub fn exec(&self, cmd: &Command) -> Result<Output> {
        run(&mut self.docker_exec(cmd), cmd.get_stdin())
    }

    pub fn spawn(&self, cmd: &Command) -> Result<Child> {
        let mut docker_exec = self.docker_exec(cmd);

        docker_exec.stdout(Stdio::piped()).stderr(Stdio::piped());

        if let Some(stdin) = cmd.get_stdin() {
            let mut temp_file = tempfile::tempfile()?;
            temp_file.write_all(stdin.as_bytes())?;
            temp_file.seek(SeekFrom::Start(0))?;
            docker_exec.stdin(Stdio::from(temp_file));
        }

        Ok(Child::new(docker_exec.spawn()?))
    }

    fn docker_exec(&self, cmd: &Command) -> process::Command {
        let mut docker_exec = StdCommand::new("docker");
        docker_exec.arg("exec");
        if cmd.get_stdin().is_some() {
            docker_exec.arg("-i");
        }
        if cmd.get_tty() {
            docker_exec.arg("--tty");
        }
        if let Some(as_) = cmd.get_as() {
            docker_exec.arg("--user");
            docker_exec.arg(as_.to_string());
        }
        docker_exec.arg(&self.id);
        docker_exec.args(cmd.get_args());
        docker_exec
    }

    pub fn cp(&self, path_in_container: &str, file_contents: &str) -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        fs::write(&mut temp_file, file_contents)?;

        let src_path = temp_file.path().display().to_string();
        let dest_path = format!("{}:{path_in_container}", self.id);

        run(
            StdCommand::new("docker").args(["cp", &src_path, &dest_path]),
            None,
        )?
        .assert_success()?;

        Ok(())
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        // running this to completion would block the current thread for several seconds so just
        // fire and forget
        let _ = StdCommand::new("docker")
            .args(["stop", &self.id])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

pub fn build_base_image() -> Result<()> {
    let repo_root = repo_root();
    let mut cmd = StdCommand::new("docker");

    cmd.args(["buildx", "build", "-t", base_image(), "--load"]);

    if env::var_os("CI").is_some() {
        cmd.args([
            "--cache-from=type=local,src=/tmp/.buildx-cache",
            "--cache-to=type=local,dest=/tmp/.buildx-cache-new,mode=max",
        ]);
    }

    match SudoUnderTest::from_env()? {
        SudoUnderTest::Ours => {
            // needed for dockerfile-specific dockerignore (e.g. `Dockerfile.dockerignore`) support
            cmd.current_dir(repo_root);
            cmd.args(["-f", "test-framework/sudo-test/src/ours.Dockerfile", "."]);
        }

        SudoUnderTest::Theirs => {
            // pass Dockerfile via stdin to not provide the repository as a build context
            let f = File::open(repo_root.join("test-framework/sudo-test/src/theirs.Dockerfile"))?;
            cmd.arg("-").stdin(Stdio::from(f));
        }
    }

    if env::var_os("SUDO_TEST_VERBOSE_DOCKER_BUILD").is_none() {
        cmd.stderr(Stdio::null()).stdout(Stdio::null());
    }

    if !cmd.status()?.success() {
        return Err("`docker build` failed".into());
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn run(cmd: &mut StdCommand, stdin: Option<&str>) -> Result<Output> {
    if let Some(stdin) = stdin {
        let mut temp_file = tempfile::tempfile()?;
        temp_file.write_all(stdin.as_bytes())?;
        temp_file.seek(SeekFrom::Start(0))?;
        cmd.stdin(Stdio::from(temp_file));
    }

    cmd.output()?.try_into()
}

fn validate_docker_id(id: &str, cmd: &StdCommand) -> Result<()> {
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
        let mut check_cmd = StdCommand::new("docker");
        let docker = Container::new(IMAGE)?;
        check_cmd.args(["ps", "--all", "--quiet", "--filter"]);
        check_cmd.arg(format!("id={}", docker.id));

        let matches = run(&mut check_cmd, None)?.stdout()?;
        assert_eq!(1, matches.lines().count());
        drop(docker);

        // wait for a bit until `stop` and `--rm` have done their work
        thread::sleep(Duration::from_secs(15));

        let matches = run(&mut check_cmd, None)?.stdout()?;
        assert_eq!(0, matches.lines().count());

        Ok(())
    }

    #[test]
    fn exec_as_root_works() -> Result<()> {
        let docker = Container::new(IMAGE)?;

        docker.exec(&Command::new("true"))?.assert_success()?;

        let output = docker.exec(&Command::new("false"))?;
        assert_eq!(Some(1), output.status.code());

        Ok(())
    }

    #[test]
    fn exec_as_user_named_root_works() -> Result<()> {
        let docker = Container::new(IMAGE)?;

        docker
            .exec(Command::new("true").as_user("root"))?
            .assert_success()
    }

    #[test]
    fn exec_as_non_root_user_works() -> Result<()> {
        let username = "ferris";

        let docker = Container::new(IMAGE)?;

        docker
            .exec(Command::new("useradd").arg(username))?
            .assert_success()?;

        docker
            .exec(Command::new("true").as_user(username))?
            .assert_success()
    }

    #[test]
    fn cp_works() -> Result<()> {
        let path = "/tmp/file";
        let expected = "Hello, world!";

        let docker = Container::new(IMAGE)?;

        docker.cp(path, expected)?;

        let actual = docker.exec(Command::new("cat").arg(path))?.stdout()?;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn stdin_works() -> Result<()> {
        let expected = "Hello, root!";
        let filename = "greeting";

        let docker = Container::new(IMAGE)?;

        docker
            .exec(Command::new("tee").arg(filename).stdin(expected))?
            .assert_success()?;

        let actual = docker.exec(Command::new("cat").arg(filename))?.stdout()?;
        assert_eq!(expected, actual);

        Ok(())
    }

    #[test]
    fn spawn_works() -> Result<()> {
        let docker = Container::new(IMAGE)?;

        let child = docker.spawn(Command::new("sh").args(["-c", "sleep 2"]))?;

        // `sh` process may not be immediately visible to `pidof` since it was spawned so wait a bit
        thread::sleep(Duration::from_millis(500));

        docker
            .exec(Command::new("pidof").arg("sh"))?
            .assert_success()?;

        child.wait()?.assert_success()?;

        let output = docker.exec(Command::new("pidof").arg("sh"))?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        Ok(())
    }
}
