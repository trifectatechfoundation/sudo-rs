use pretty_assertions::assert_eq;
use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_FERRIS_ALL_NOPASSWD};

#[test]
fn user_can_read_file_owned_by_root() -> Result<()> {
    let expected = "hello";
    let path = "/root/file";
    let username = "ferris";
    let env = Env::new(SUDOERS_FERRIS_ALL_NOPASSWD)
        .user(username, &[])
        .text_file(path, "root:root", "000", expected)
        .build()?;

    let actual = Command::new("sudo")
        .args(["cat", path])
        .as_user(username)
        .exec(&env)?
        .stdout()?;
    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn user_can_write_file_owned_by_root() -> Result<()> {
    let path = "/root/file";
    let username = "ferris";
    let env = Env::new(SUDOERS_FERRIS_ALL_NOPASSWD)
        .user(username, &[])
        .text_file(path, "root:root", "000", "")
        .build()?;

    Command::new("sudo")
        .args(["rm", path])
        .as_user(username)
        .exec(&env)?
        .assert_success()
}

#[test]
#[ignore]
fn user_can_execute_file_owned_by_root() -> Result<()> {
    let path = "/root/file";
    let username = "ferris";
    let env = Env::new(SUDOERS_FERRIS_ALL_NOPASSWD)
        .user(username, &[])
        .text_file(
            path,
            "root:root",
            "100",
            r#"#!/bin/sh
exit 0"#,
        )
        .build()?;

    Command::new("sudo")
        .arg(path)
        .as_user(username)
        .exec(&env)?
        .assert_success()
}
