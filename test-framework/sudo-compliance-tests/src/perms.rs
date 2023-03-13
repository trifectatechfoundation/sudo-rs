use pretty_assertions::assert_eq;
use sudo_test::{As, EnvBuilder};

use crate::{Result, SUDOERS_FERRIS_ALL_NOPASSWD};

#[test]
fn user_can_read_file_owned_by_root() -> Result<()> {
    let expected = "hello";
    let path = "/root/file";
    let env = EnvBuilder::default()
        .user("ferris", &[])
        .sudoers(SUDOERS_FERRIS_ALL_NOPASSWD)
        .text_file(path, "root:root", "000", expected)
        .build()?;

    let stdout = env.stdout(&["sudo", "cat", path], As::User { name: "ferris" }, None)?;
    assert_eq!(expected, stdout);

    Ok(())
}

#[test]
fn user_can_write_file_owned_by_root() -> Result<()> {
    let path = "/root/file";
    let env = EnvBuilder::default()
        .user("ferris", &[])
        .sudoers(SUDOERS_FERRIS_ALL_NOPASSWD)
        .text_file(path, "root:root", "000", "")
        .build()?;

    let output = env.exec(&["sudo", "rm", path], As::User { name: "ferris" }, None)?;
    assert!(output.status.success());

    Ok(())
}

#[test]
#[ignore]
fn user_can_execute_file_owned_by_root() -> Result<()> {
    let path = "/root/file";
    let env = EnvBuilder::default()
        .user("ferris", &[])
        .sudoers(SUDOERS_FERRIS_ALL_NOPASSWD)
        .text_file(
            path,
            "root:root",
            "100",
            r#"#!/bin/sh
exit 0"#,
        )
        .build()?;

    let output = env.exec(&["sudo", path], As::User { name: "ferris" }, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}
