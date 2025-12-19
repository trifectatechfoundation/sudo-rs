use core::str;
use std::{
    env::{self, consts::OS},
    fs::{self, File},
    io::{ErrorKind, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{self, Command as StdCommand, Stdio},
};

use tempfile::NamedTempFile;

use crate::{base_image, Result, SudoUnderTest};

pub use self::command::{As, Child, Command, Output};

mod command;

const DOCKER_RUN_COMMAND: &[&str] = &["sleep", "1000d"];

pub struct Container {
    id: String,
}

fn docker_command() -> StdCommand {
    if cfg!(target_os = "freebsd") {
        let mut cmd = StdCommand::new("sudo");
        cmd.arg("podman");
        cmd
    } else {
        StdCommand::new("docker")
    }
}

fn docker_build_command(tag: &str) -> StdCommand {
    if cfg!(target_os = "freebsd") {
        let mut cmd = StdCommand::new("sudo");
        cmd.args(["podman", "build", "-t", base_image()]);
        cmd
    } else {
        let mut cmd = StdCommand::new("docker");
        cmd.args(["buildx", "build", "-t", tag, "--load"]);

        if env::var_os("CI").is_some() {
            cmd.args([
                "--cache-from=type=local,src=/tmp/.buildx-cache",
                "--cache-to=type=local,dest=/tmp/.buildx-cache-new,mode=max",
            ]);
        }

        cmd
    }
}

impl Container {
    #[cfg(test)]
    fn new(image: &str) -> Self {
        Self::new_with_hostname(
            image,
            None,
            #[cfg(feature = "apparmor")]
            None,
        )
    }

    pub fn new_with_hostname(
        image: &str,
        hostname: Option<&str>,
        #[cfg(feature = "apparmor")] apparmor_profile: Option<&str>,
    ) -> Self {
        let mut docker_run = docker_command();
        docker_run.args(["run", "--detach"]);
        #[cfg(feature = "apparmor")]
        if let Some(profile) = apparmor_profile {
            docker_run.arg("--security-opt");
            docker_run.arg(format!("apparmor={profile}"));
        }
        // Disable network access for the containers. This removes the overhead
        // of setting up a new network namespace and associated firewall rule
        // adjustments. On FreeBSD it seems to introduce extra overhead however.
        if cfg!(not(target_os = "freebsd")) {
            docker_run.arg("--net=none");
        }
        if let Some(hostname) = hostname {
            docker_run.args(["--hostname", hostname]);
        }
        docker_run.args(["--rm", image]).args(DOCKER_RUN_COMMAND);
        let id = run(&mut docker_run, None).stdout();
        validate_docker_id(&id, &docker_run);

        Container { id }
    }

    #[track_caller]
    pub fn output(&self, cmd: &Command) -> Output {
        run(&mut self.docker_exec(cmd), cmd.get_stdin())
    }

    #[track_caller]
    pub fn spawn(&self, cmd: &Command) -> Child {
        let mut docker_exec = self.docker_exec(cmd);

        docker_exec.stdout(Stdio::piped()).stderr(Stdio::piped());

        let res = (|| -> Result<Child> {
            if let Some(stdin) = cmd.get_stdin() {
                let mut temp_file = tempfile::tempfile()?;
                temp_file.write_all(stdin.as_bytes())?;
                temp_file.seek(SeekFrom::Start(0))?;
                docker_exec.stdin(Stdio::from(temp_file));
            }

            Ok(Child::new(docker_exec.spawn()?))
        })();
        match res {
            Ok(child) => child,
            Err(err) => panic!("running `{docker_exec:?}` failed: {err}"),
        }
    }

    fn docker_exec(&self, cmd: &Command) -> process::Command {
        let mut docker_exec = docker_command();
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

    pub fn cp(&self, path_in_container: &str, file_contents: &str) {
        let mut temp_file = NamedTempFile::new().unwrap();
        fs::write(&mut temp_file, file_contents).unwrap();

        let src_path = temp_file.path().display().to_string();
        let dest_path = format!("{}:{path_in_container}", self.id);

        run(docker_command().args(["cp", &src_path, &dest_path]), None).assert_success();
    }

    fn copy_profraw_data(&mut self, profraw_dir: impl AsRef<Path>) {
        let profraw_dir = profraw_dir.as_ref();
        fs::create_dir_all(profraw_dir).unwrap();

        self.output(Command::new("sh").args([
            "-c",
            "mkdir /tmp/profraw; find / -name '*.profraw' -exec cp {} /tmp/profraw/ \\; || true",
        ]))
        .assert_success();

        let src_path = format!("{}:/tmp/profraw", self.id);
        let dst_path = profraw_dir.join(&self.id).display().to_string();
        run(docker_command().args(["cp", &src_path, &dst_path]), None).assert_success();
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        if let Ok(dir) = env::var("SUDO_TEST_PROFRAW_DIR") {
            self.copy_profraw_data(dir);
        }

        let _ = docker_command()
            .args(["kill", &self.id])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

pub fn build_base_image() {
    let repo_root = repo_root();
    let mut cmd = docker_build_command(base_image());

    match SudoUnderTest::from_env() {
        SudoUnderTest::Ours => {
            #[allow(clippy::vec_init_then_push)]
            let sudo_build_features: String =
                env::var("SUDO_BUILD_FEATURES").unwrap_or_else(|_| {
                    [
                        "gettext",
                        #[cfg(not(target_os = "freebsd"))]
                        "pam-login",
                        #[cfg(feature = "apparmor")]
                        "apparmor",
                    ]
                    .join(",")
                });

            // On FreeBSD we build sudo-rs outside of the container. There are no pre-made FreeBSD
            // Rust container images and unlike on Linux we intend to run the exact same FreeBSD
            // version outside of the container and inside.
            if cfg!(target_os = "freebsd") {
                // Build sudo-rs
                let mut cargo_cmd = StdCommand::new("cargo");
                cargo_cmd.env("RUSTFLAGS", "-L /usr/local/lib").args([
                    "build",
                    "--locked",
                    "--features",
                    &sudo_build_features,
                    "--bins",
                ]);
                cargo_cmd.current_dir(&repo_root);
                if !cargo_cmd.status().unwrap().success() {
                    eprintln!(
                        "`cargo build --locked --features \"{sudo_build_features}\" --bins` failed"
                    );
                    // Panic without panic message and backtrace
                    std::panic::resume_unwind(Box::new(()));
                }

                // Copy all binaries to a single place where the Dockerfile will find them
                let target_debug_dir = repo_root.join("target").join("debug");
                let build_dir = repo_root.join("target").join("build");
                match fs::create_dir(&build_dir) {
                    Ok(()) => {}
                    Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
                    Err(e) => panic!("failed to create build dir: {e}"),
                }
                for f in ["sudo", "su", "visudo"] {
                    fs::copy(target_debug_dir.join(f), build_dir.join(f)).unwrap();
                }
            }

            // set the build features argument for the docker container
            let sudo_build_features_arg = format!("SUDO_BUILD_FEATURES={sudo_build_features}");
            cmd.args(["--build-arg", &sudo_build_features_arg]);

            // needed for dockerfile-specific dockerignore (e.g. `Dockerfile.dockerignore`) support
            cmd.current_dir(repo_root);
            cmd.args([
                "-f",
                &format!("test-framework/sudo-test/src/ours.{OS}.Dockerfile"),
                ".",
            ]);
        }

        SudoUnderTest::Theirs => {
            // pass Dockerfile via stdin to not provide the repository as a build context
            let f = File::open(
                repo_root
                    .join("test-framework/sudo-test/src")
                    .join(format!("theirs.{OS}.Dockerfile")),
            )
            .unwrap();
            cmd.arg("-").stdin(Stdio::from(f));
        }
    }

    if !cmd.status().unwrap().success() {
        eprintln!("`{cmd:?}` failed");
        // Panic without panic message and backtrace
        std::panic::resume_unwind(Box::new(()));
    }
}

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

#[track_caller]
fn run(cmd: &mut StdCommand, stdin: Option<&str>) -> Output {
    let res = (|| -> Result<Output> {
        if let Some(stdin) = stdin {
            let mut temp_file = tempfile::tempfile()?;
            temp_file.write_all(stdin.as_bytes())?;
            temp_file.seek(SeekFrom::Start(0))?;
            cmd.stdin(Stdio::from(temp_file));
        }

        cmd.output()?.try_into()
    })();
    match res {
        Ok(output) => output,
        Err(err) => panic!("running `{cmd:?}` failed: {err}"),
    }
}

fn validate_docker_id(id: &str, cmd: &StdCommand) {
    if id.chars().any(|c| !c.is_ascii_hexdigit()) {
        panic!("`{cmd:?}` return what appears to be an invalid docker id: {id}");
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    #[cfg(target_os = "linux")]
    const IMAGE: &str = "ubuntu:22.04";

    #[cfg(target_os = "freebsd")]
    const IMAGE: &str = "dougrabson/freebsd14-small:latest";

    #[test]
    fn eventually_removes_container_on_drop() {
        let mut check_cmd = StdCommand::new("docker");
        let docker = Container::new(IMAGE);
        check_cmd.args(["ps", "--all", "--quiet", "--filter"]);
        check_cmd.arg(format!("id={}", docker.id));

        let matches = run(&mut check_cmd, None).stdout();
        assert_eq!(1, matches.lines().count());
        drop(docker);

        // wait for a bit until `stop` and `--rm` have done their work
        thread::sleep(Duration::from_secs(15));

        let matches = run(&mut check_cmd, None).stdout();
        assert_eq!(0, matches.lines().count());
    }

    #[test]
    fn exec_as_root_works() {
        let docker = Container::new(IMAGE);

        docker.output(&Command::new("true")).assert_success();

        let output = docker.output(&Command::new("false"));
        assert_eq!(Some(1), output.status.code());
    }

    #[test]
    fn exec_as_user_named_root_works() {
        let docker = Container::new(IMAGE);

        docker
            .output(Command::new("true").as_user("root"))
            .assert_success();
    }

    #[test]
    fn exec_as_non_root_user_works() {
        let username = "ferris";

        let docker = Container::new(IMAGE);

        if cfg!(target_os = "linux") {
            docker
                .output(Command::new("useradd").arg(username))
                .assert_success();
        } else if cfg!(target_os = "freebsd") {
            docker
                .output(Command::new("pw").args(["useradd", username]))
                .assert_success();
        } else {
            todo!()
        }

        docker
            .output(Command::new("true").as_user(username))
            .assert_success();
    }

    #[test]
    fn cp_works() {
        let path = "/tmp/file";
        let expected = "Hello, world!";

        let docker = Container::new(IMAGE);

        docker.cp(path, expected);

        let actual = docker.output(Command::new("cat").arg(path)).stdout();
        assert_eq!(expected, actual);
    }

    #[test]
    fn stdin_works() {
        let expected = "Hello, root!";
        let filename = "greeting";

        let docker = Container::new(IMAGE);

        docker
            .output(Command::new("tee").arg(filename).stdin(expected))
            .assert_success();

        let actual = docker.output(Command::new("cat").arg(filename)).stdout();
        assert_eq!(expected, actual);
    }

    #[test]
    fn spawn_works() {
        let docker = Container::new(IMAGE);

        let child = docker.spawn(Command::new("sh").args(["-c", "sleep 2"]));

        // `sh` process may not be immediately visible to `pidof` since it was spawned so wait a bit
        thread::sleep(Duration::from_millis(500));

        docker
            .output(Command::new("pidof").arg("sh"))
            .assert_success();

        child.wait().assert_success();

        let output = docker.output(Command::new("pidof").arg("sh"));

        output.assert_exit_code(1);
    }
}
