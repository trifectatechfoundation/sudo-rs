//! Scenarios where a password does not need to be provided

use sudo_test::{Command, Env, User};

use crate::{Result, GROUPNAME, SUDOERS_NO_LECTURE, SUDOERS_ROOT_ALL, USERNAME};

// NOTE all these tests assume that the invoking user passes the sudoers file 'User_List' criteria

// man sudoers > User Authentication:
// "A password is not required if the invoking user is root"
#[test]
fn user_is_root() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL).build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

// man sudoers > User Authentication:
// "A password is not required if (..) the target user is the same as the invoking user"
#[test]
fn user_as_themselves() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .args(["-u", USERNAME, "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn user_as_their_own_group() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .group(GROUPNAME)
        .user(User(USERNAME).secondary_group(GROUPNAME))
        .build()?;

    Command::new("sudo")
        .args(["-g", GROUPNAME, "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn nopasswd_tag() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) NOPASSWD: ALL"))
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn nopasswd_tag_for_command() -> Result<()> {
    let env = Env(format!(
        "{USERNAME}    ALL=(ALL:ALL) NOPASSWD: /usr/bin/true"
    ))
    .user(USERNAME)
    .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
#[ignore = "gh530"]
fn run_sudo_l_flag_without_pwd_if_one_nopasswd_is_set() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: /bin/true, PASSWD: /bin/ls")
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo")
        .arg("-l")
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success());

    let actual = output.stdout()?;
    if sudo_test::is_original_sudo() {
        assert_contains!(
            actual,
            format!("User {USERNAME} may run the following commands")
        );
    } else {
        assert_contains!(
            actual,
            format!("authentication failed: I'm sorry {USERNAME}. I'm afraid I can't do that")
        );
    }

    Ok(())
}

#[test]
#[ignore = "gh439"]
fn run_sudo_v_flag_without_pwd_if_nopasswd_is_set_for_all_users_entries() -> Result<()> {
    let env = Env(format!(
        "{USERNAME}    ALL=(ALL:ALL) NOPASSWD: /bin/true, /bin/ls"
    ))
    .user(USERNAME)
    .build()?;

    Command::new("sudo")
        .arg("-v")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
#[ignore = "gh439"]
fn v_flag_without_pwd_fails_if_nopasswd_is_not_set_for_all_users_entries() -> Result<()> {
    let env = Env([
        "ALL ALL=(ALL:ALL) NOPASSWD: /bin/true, PASSWD: /bin/ls",
        SUDOERS_NO_LECTURE,
    ])
    .user(USERNAME)
    .build()?;

    let output = Command::new("sudo")
        .args(["-S", "-v"])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_contains!(
            stderr,
            format!("[sudo] password for {USERNAME}: \nsudo: no password was provided\nsudo: a password is required")
        );
    } else {
        assert_contains!(
            stderr,
            "[Sudo: authenticate] Password: sudo: Authentication failed, try again.\n[Sudo: authenticate] Password: sudo: Authentication failed, try again.\n[Sudo: authenticate] Password: sudo-rs: Maximum 3 incorrect authentication attempts"
        );
    }

    Ok(())
}
