//! Test the NOEXEC tag and the noexec option

use sudo_test::{Command, Env, BIN_TRUE};

use crate::{Result, USERNAME};

#[test]
fn sanity_check() -> Result<()> {
    let env = Env("Defaults noexec\nALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .user(USERNAME)
        .build();

    Command::new("sudo")
        .arg("/usr/bin/true")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    Ok(())
}

#[test]
fn exec_denied() -> Result<()> {
    let env = Env("Defaults noexec\nALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .user(USERNAME)
        .build();

    let output = Command::new("sudo")
        .args(["sh", "-c", BIN_TRUE])
        .as_user(USERNAME)
        .output(&env);

    assert!(
        !output.status().success(),
        "stdout:\n{}\n\nstderr:\n{}",
        output.stdout_unchecked(),
        output.stderr(),
    );

    assert!(output.stderr().contains("Permission denied"));

    Ok(())
}

#[test]
fn exec_denied_second_time() -> Result<()> {
    let env = Env("Defaults noexec\nALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .user(USERNAME)
        .build();

    let output = Command::new("sudo")
        .args(["sh", "-c"])
        .arg(format!("{BIN_TRUE} || {BIN_TRUE}"))
        .as_user(USERNAME)
        .output(&env);

    assert!(
        !output.status().success(),
        "stdout:\n{}\n\nstderr:\n{}",
        output.stdout_unchecked(),
        output.stderr(),
    );

    assert_eq!(
        output.stderr(),
        "sh: 1: /usr/bin/true: Permission denied
sh: 1: /usr/bin/true: Permission denied"
    );

    Ok(())
}

#[test]
fn exec_denied_noexec_tag() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: NOEXEC: ALL")
        .user(USERNAME)
        .build();

    let output = Command::new("sudo")
        .args(["sh", "-c", BIN_TRUE])
        .as_user(USERNAME)
        .output(&env);

    assert!(
        !output.status().success(),
        "stdout:\n{}\n\nstderr:\n{}",
        output.stdout_unchecked(),
        output.stderr(),
    );

    assert!(
        output.stderr().contains("Permission denied"),
        "stderr:\n{}",
        output.stderr(),
    );

    Ok(())
}

#[test]
fn exec_overrides_noexec_default() -> Result<()> {
    let env = Env("Defaults noexec\nALL ALL=(ALL:ALL) NOPASSWD: EXEC: ALL")
        .user(USERNAME)
        .build();

    Command::new("sudo")
        .args(["sh", "-c", BIN_TRUE])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    Ok(())
}

#[test]
fn no_use_pty_works() -> Result<()> {
    let env = Env("Defaults noexec, !use_pty\nALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .user(USERNAME)
        .build();

    let output = Command::new("sudo")
        .args(["sh", "-c", BIN_TRUE])
        .as_user(USERNAME)
        .output(&env);

    assert!(
        !output.status().success(),
        "stdout:\n{}\n\nstderr:\n{}",
        output.stdout_unchecked(),
        output.stderr(),
    );

    assert!(output.stderr().contains("Permission denied"));

    Ok(())
}
