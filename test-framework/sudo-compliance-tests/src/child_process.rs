use pretty_assertions::assert_eq;
use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_ROOT_ALL_NOPASSWD};

mod signal_handling;

#[test]
fn sudo_forwards_childs_exit_code() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let expected = 42;
    let output = Command::new("sudo")
        .args(["sh", "-c"])
        .arg(format!("exit {expected}"))
        .output(&env)?;
    assert_eq!(Some(expected), output.status().code());

    Ok(())
}

#[test]
fn sudo_forwards_childs_stdout() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let expected = "hello";
    let output = Command::new("sudo").args(["echo", expected]).output(&env)?;
    assert!(output.stderr().is_empty());
    assert_eq!(expected, output.stdout()?);

    Ok(())
}

#[test]
fn sudo_forwards_childs_stderr() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let expected = "hello";
    let output = Command::new("sudo")
        .args(["sh", "-c"])
        .arg(format!(">&2 echo {expected}"))
        .output(&env)?;
    assert_eq!(expected, output.stderr());
    assert!(output.stdout()?.is_empty());

    Ok(())
}

#[test]
fn sudo_forwards_stdin_to_child() -> Result<()> {
    let expected = "hello";
    let path = "/root/file";
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    Command::new("sudo")
        .args(["tee", path])
        .stdin(expected)
        .output(&env)?
        .assert_success()?;

    let actual = Command::new("cat").arg(path).output(&env)?.stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}
