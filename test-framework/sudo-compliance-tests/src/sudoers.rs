use sudo_test::{Command, Env, TextFile, User};

use crate::{Result, PASSWORD, SUDOERS_ROOT_ALL_NOPASSWD, USERNAME};

mod cmnd;
mod host_list;
mod run_as;
mod secure_path;
mod user_list;

#[test]
fn cannot_sudo_if_sudoers_file_is_world_writable() -> Result<()> {
    let env = Env(TextFile(SUDOERS_ROOT_ALL_NOPASSWD).chmod("446")).build()?;

    let output = Command::new("sudo").arg("true").exec(&env)?;
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "/etc/sudoers is world writable");
    }

    Ok(())
}

#[test]
fn cannot_sudo_if_sudoers_file_is_group_writable() -> Result<()> {
    let env = Env(TextFile(SUDOERS_ROOT_ALL_NOPASSWD)
        .chmod("464")
        .chown("root:1234"))
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    let output = Command::new("sudo").arg("true").exec(&env)?;
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "/etc/sudoers is owned by gid 1234, should be 0"
        );
    }

    Ok(())
}

#[test]
fn can_sudo_if_sudoers_file_is_owner_writable() -> Result<()> {
    let env = Env(TextFile(SUDOERS_ROOT_ALL_NOPASSWD).chmod("644")).build()?;

    let output = Command::new("sudo").arg("true").exec(&env)?;
    assert_eq!(Some(0), output.status().code());

    Ok(())
}

#[test]
fn cannot_sudo_if_sudoers_file_is_not_owned_by_root() -> Result<()> {
    let env = Env(TextFile(SUDOERS_ROOT_ALL_NOPASSWD).chown("1234:root"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo").arg("true").exec(&env)?;
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "/etc/sudoers is owned by uid 1234, should be 0"
        );
    }

    Ok(())
}

#[test]
fn user_specifications_evaluated_bottom_to_top() -> Result<()> {
    let env = Env(format!(
        r#"{USERNAME} ALL=(ALL:ALL) NOPASSWD: ALL
{USERNAME} ALL=(ALL:ALL) ALL"#
    ))
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .exec(&env)?;
    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "no password was provided");
    }

    Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .exec(&env)?
        .assert_success()
}

#[test]
fn accepts_sudoers_file_that_has_no_trailing_newline() -> Result<()> {
    let env = Env(TextFile(SUDOERS_ROOT_ALL_NOPASSWD).no_trailing_newline())
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}
