use pretty_assertions::assert_eq;
use sudo_test::{Command, Env, TextFile};

use crate::{Result, SUDOERS_USER_ALL_NOPASSWD, USERNAME};

#[test]
fn user_can_read_file_owned_by_root() -> Result<()> {
    let expected = "hello";
    let path = "/root/file";
    let env = Env(SUDOERS_USER_ALL_NOPASSWD)
        .user(USERNAME)
        .file(path, expected)
        .build()?;

    let actual = Command::new("sudo")
        .args(["cat", path])
        .as_user(USERNAME)
        .output(&env)?
        .stdout()?;
    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn user_can_write_file_owned_by_root() -> Result<()> {
    let path = "/root/file";
    let env = Env(SUDOERS_USER_ALL_NOPASSWD)
        .user(USERNAME)
        .file(path, "")
        .build()?;

    Command::new("sudo")
        .args(["rm", path])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn user_can_execute_file_owned_by_root() -> Result<()> {
    let path = "/root/file";
    let env = Env(SUDOERS_USER_ALL_NOPASSWD)
        .user(USERNAME)
        .file(
            path,
            TextFile(
                r#"#!/bin/sh
exit 0"#,
            )
            .chmod("100"),
        )
        .build()?;

    Command::new("sudo")
        .arg(path)
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}
