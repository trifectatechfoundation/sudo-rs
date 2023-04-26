use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};
use sudo_test::{Command, Env, TextFile};

#[test]
fn cwd_not_set_cannot_change_dir() -> Result<()> {
    let env = Env(TextFile(SUDOERS_ALL_ALL_NOPASSWD)).build()?;

    let output = Command::new("sudo")
        .args(["--chdir", "/root", "pwd"])
        .exec(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert_eq!(false, output.status().success());
    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "you are not permitted to use the -D option with /bin/pwd"
        );
    }

    Ok(())
}

#[test]
fn cwd_set_to_glob_change_dir() -> Result<()> {
    let env = Env(TextFile("ALL ALL=(ALL:ALL) CWD=* NOPASSWD: ALL")).build()?;
    let output = Command::new("sh")
        .args(["-c", "cd /; sudo --chdir /root pwd"])
        .exec(&env)?;
    assert_eq!(Some(0), output.status().code());
    assert_eq!(true, output.status().success());
    assert_contains!(output.stdout()?, "/root");

    Ok(())
}

#[test]
fn cwd_fails_for_non_existent_dirs() -> Result<()> {
    let env = Env(TextFile("ALL ALL=(ALL:ALL) CWD=* NOPASSWD: ALL")).build()?;
    let output = Command::new("sudo")
        .args([
            "--chdir",
            "/path/to/nowhere",
            "sh",
            "-c",
            "echo >&2 'avocado'",
        ])
        .exec(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert_eq!(false, output.status().success());
    let stderr = output.stderr();
    assert_contains!(
        stderr,
        "unable to change directory to /path/to/nowhere: No such file or directory"
    );
    assert!(!stderr.contains("avocado"));

    Ok(())
}

#[test]
fn cwd_with_login_fails_for_non_existent_dirs() -> Result<()> {
    let env = Env(TextFile("ALL ALL=(ALL:ALL) CWD=* NOPASSWD: ALL"))
        .user(USERNAME)
        .build()?;
    let output = Command::new("sudo")
        .args([
            "-u",
            USERNAME,
            "-i",
            "--chdir",
            "/path/to/nowhere",
            "sh",
            "-c",
            "echo >&2 'avocado'",
        ])
        .exec(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert_eq!(false, output.status().success());
    let stderr = output.stderr();
    assert_contains!(
        stderr,
        "unable to change directory to /path/to/nowhere: No such file or directory"
    );
    assert!(!stderr.contains("avocado"));

    Ok(())
}
