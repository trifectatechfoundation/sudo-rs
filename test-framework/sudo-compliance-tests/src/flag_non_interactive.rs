use crate::{
    Result, OG_SUDO_STANDARD_LECTURE, PASSWORD, SUDOERS_ROOT_ALL, SUDOERS_USER_ALL_ALL,
    SUDOERS_USER_ALL_NOPASSWD, USERNAME,
};

use sudo_test::{Command, Env, User};

/* cases where password input is expected */
#[test]
#[ignore]
fn fails_if_password_needed() -> Result<()> {
    let env = Env(SUDOERS_USER_ALL_ALL).user(USERNAME).build()?;

    let output = Command::new("sudo")
        .args(["-n", "true"])
        .as_user(USERNAME)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    let password_prompt = if sudo_test::is_original_sudo() {
        "password for ferris"
    } else {
        "Password:"
    };
    assert_not_contains!(stderr, password_prompt);

    let diagnostic = "sudo: a password is required";
    assert_contains!(stderr, diagnostic);

    Ok(())
}

#[test]
#[ignore]
fn flag_remove_timestamp_plus_command_fails() -> Result<()> {
    let env = Env(SUDOERS_USER_ALL_ALL).user(USERNAME).build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true 2>/dev/null; sudo -n -k true"
        ))
        .as_user(USERNAME)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    let password_prompt = if sudo_test::is_original_sudo() {
        "password for ferris"
    } else {
        "Password:"
    };
    assert_not_contains!(stderr, password_prompt);

    let diagnostic = "sudo: a password is required";
    assert_contains!(stderr, diagnostic);

    Ok(())
}

/* cases where password input is not required */
#[test]
fn root() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL).build()?;

    Command::new("sudo")
        .args(["-n", "true"])
        .exec(&env)?
        .assert_success()
}

#[test]
fn nopasswd() -> Result<()> {
    let env = Env(SUDOERS_USER_ALL_NOPASSWD).user(USERNAME).build()?;

    Command::new("sudo")
        .args(["-n", "true"])
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}

#[test]
fn cached_credential() -> Result<()> {
    let env = Env(SUDOERS_USER_ALL_ALL)
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S true; sudo -n true"))
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}

/* misc */
#[test]
fn lecture_is_not_shown() -> Result<()> {
    let env = Env(SUDOERS_USER_ALL_ALL).user(USERNAME).build()?;

    let output = Command::new("sudo")
        .args(["-n", "true"])
        .as_user(USERNAME)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    assert_not_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);

    Ok(())
}
