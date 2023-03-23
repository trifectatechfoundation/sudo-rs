//! Test the run_as component of the user specification: <user> ALL=(<run_as>) ALL`

use sudo_test::{Command, Env, User};

use crate::{Result, GROUPNAME, USERNAME};

// "If both Runas_Lists are empty, the command may only be run as the invoking user."
#[test]
#[ignore]
fn when_empty_then_implicit_as_self_is_allowed() -> Result<()> {
    let env = Env("ALL ALL=() NOPASSWD: ALL").user(USERNAME).build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_empty_then_explicit_as_self_is_allowed() -> Result<()> {
    let env = Env("ALL ALL=() NOPASSWD: ALL").user(USERNAME).build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-u", user, "true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_empty_then_as_someone_else_is_not_allowed() -> Result<()> {
    let env = Env("ALL ALL=() NOPASSWD: ALL").user(USERNAME).build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "true"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "user root is not allowed to execute '/bin/true' as ferris"
        );
    }

    Ok(())
}

#[test]
fn when_empty_then_as_own_group_is_allowed() -> Result<()> {
    let env = Env("ALL ALL=() NOPASSWD: ALL")
        .group(USERNAME)
        .user(User(USERNAME).group(USERNAME))
        .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-g", user, "true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_specific_user_then_as_that_user_is_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=({USERNAME}) NOPASSWD: ALL"))
        .user(USERNAME)
        .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-u", USERNAME, "true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_specific_user_then_as_a_different_user_is_not_allowed() -> Result<()> {
    let env = Env("ALL ALL=(ferris) NOPASSWD: ALL")
        .user("ferris")
        .user("ghost")
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", "ghost", "true"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "user root is not allowed to execute '/bin/true' as ghost"
        );
    }

    Ok(())
}

#[test]
fn when_specific_user_then_as_self_is_not_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=({USERNAME}) NOPASSWD: ALL")).build()?;

    let output = Command::new("sudo").args(["true"]).exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "user root is not allowed to execute '/bin/true' as root"
        );
    }

    Ok(())
}

#[test]
fn when_specific_group_then_as_that_group_is_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=(:{GROUPNAME}) NOPASSWD: ALL"))
        .user(USERNAME)
        .group(GROUPNAME)
        .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-g", GROUPNAME, "true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_specific_group_then_as_a_different_group_is_not_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=(:{GROUPNAME}) NOPASSWD: ALL"))
        .group(GROUPNAME)
        .group("ghosts")
        .build()?;

    let output = Command::new("sudo")
        .args(["-g", "ghosts", "true"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "user root is not allowed to execute '/bin/true' as root:ghosts"
        );
    }

    Ok(())
}
