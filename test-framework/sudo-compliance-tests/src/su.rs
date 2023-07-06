use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

mod cli;
mod env;
mod flag_command;
mod flag_group;
mod flag_login;
mod flag_preserve_environment;
mod flag_pty;
mod flag_shell;
mod flag_supp_group;
mod flag_whitelist_environment;
mod pam;
mod syslog;

#[test]
fn default_target_is_root() -> Result<()> {
    let env = Env("").build()?;

    let actual = Command::new("su")
        .args(["-c", "whoami"])
        .output(&env)?
        .stdout()?;

    let expected = "root";
    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn explicit_target_user() -> Result<()> {
    let env = Env("").user(USERNAME).build()?;

    let actual = Command::new("su")
        .args(["-c", "whoami", USERNAME])
        .output(&env)?
        .stdout()?;

    assert_eq!(USERNAME, actual);

    Ok(())
}

#[test]
fn target_user_must_exist_in_passwd_db() -> Result<()> {
    let env = Env("").build()?;

    let output = Command::new("su")
        .args(["-c", "true", USERNAME])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("user {USERNAME} does not exist or the user entry does not contain all the required fields")
    } else {
        format!("user '{USERNAME}' not found")
    };

    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn required_password_is_target_users_pass() -> Result<()> {
    let invoking_user = "ferris";
    let target_user_name = "ghost";
    let target_user_password = PASSWORD;
    let env = Env("")
        .user(invoking_user)
        .user(User(target_user_name).password(target_user_password))
        .build()?;

    let actual = Command::new("su")
        .args(["-c", "whoami", target_user_name])
        .stdin(target_user_password)
        .as_user(invoking_user)
        .output(&env)?
        .stdout()?;

    assert_eq!(target_user_name, actual);

    Ok(())
}

#[test]
fn required_password_is_target_users_fail() -> Result<()> {
    let target_user = "ghost";
    let env = Env("")
        .user(User(USERNAME).password(PASSWORD))
        .user(target_user)
        .build()?;

    let output = Command::new("su")
        .args(["-c", "true", target_user])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "Authentication failure"
    } else {
        "Maximum 3 incorrect authentication attempts"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn nopasswd_root() -> Result<()> {
    let env = Env("").user(USERNAME).build()?;

    Command::new("su")
        .args(["-c", "true", USERNAME])
        .output(&env)?
        .assert_success()
}

#[test]
fn password_is_required_when_target_user_is_self() -> Result<()> {
    let env = Env("").user(USERNAME).build()?;

    let output = Command::new("su")
        .args(["-c", "true", USERNAME])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "Authentication failure"
    } else {
        "Maximum 3 incorrect authentication attempts"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}
