//! Test the first component of the user specification: `<user_list> ALL=(ALL:ALL) ALL`

use pretty_assertions::assert_eq;
use sudo_test::{Command, Env};

use crate::Result;

#[test]
fn no_match() -> Result<()> {
    let env = Env::new("").build()?;

    let output = Command::new("sudo").arg("true").exec(&env)?;
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "root is not in the sudoers file");
    }

    Ok(())
}

#[test]
fn all() -> Result<()> {
    let username = "ferris";
    let env = Env::new("ALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .user(username, &[])
        .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()?;

    Command::new("sudo")
        .arg("true")
        .as_user(username)
        .exec(&env)?
        .assert_success()
}

#[test]
fn user_name() -> Result<()> {
    let env = Env::new("root ALL=(ALL:ALL) NOPASSWD: ALL").build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}

#[test]
fn user_id() -> Result<()> {
    let env = Env::new("#0 ALL=(ALL:ALL) NOPASSWD: ALL").build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}

#[test]
fn group_name() -> Result<()> {
    let env = Env::new("%root ALL=(ALL:ALL) NOPASSWD: ALL").build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}

#[test]
fn group_id() -> Result<()> {
    let env = Env::new("%#0 ALL=(ALL:ALL) NOPASSWD: ALL").build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}
