use crate::{
    OG_SUDO_STANDARD_LECTURE, PASSWORD, SUDOERS_ROOT_ALL, SUDOERS_USER_ALL_ALL,
    SUDOERS_USER_ALL_NOPASSWD, USERNAME,
};

use sudo_test::{Command, Env, User};

/* cases where password input is expected */
#[test]
fn fails_if_password_needed() {
    let env = Env(SUDOERS_USER_ALL_ALL).user(USERNAME).build();

    let output = Command::new("sudo")
        .args(["-n", "true"])
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    let password_prompt = if sudo_test::is_original_sudo() {
        "password for ferris"
    } else {
        "Password:"
    };
    assert_not_contains!(stderr, password_prompt);

    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: a password is required"
    } else {
        "interactive authentication is required"
    };
    assert_contains!(stderr, diagnostic);
}

#[test]
fn flag_remove_timestamp_plus_command_fails() {
    let env = Env(SUDOERS_USER_ALL_ALL).user(USERNAME).build();

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true 2>/dev/null; sudo -n -k true && true"
        ))
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    let password_prompt = if sudo_test::is_original_sudo() {
        "password for ferris"
    } else {
        "Password:"
    };
    assert_not_contains!(stderr, password_prompt);

    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: a password is required"
    } else {
        "interactive authentication is required"
    };
    assert_contains!(stderr, diagnostic);
}

/* cases where password input is not required */
#[test]
fn root() {
    let env = Env(SUDOERS_ROOT_ALL).build();

    Command::new("sudo")
        .args(["-n", "true"])
        .output(&env)
        .assert_success();
}

#[test]
fn nopasswd() {
    let env = Env(SUDOERS_USER_ALL_NOPASSWD).user(USERNAME).build();

    Command::new("sudo")
        .args(["-n", "true"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn cached_credential() {
    let env = Env(SUDOERS_USER_ALL_ALL)
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -n true && true"
        ))
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

/* misc */
#[test]
fn lecture_is_not_shown() {
    let env = Env(SUDOERS_USER_ALL_ALL).user(USERNAME).build();

    let output = Command::new("sudo")
        .args(["-n", "true"])
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    assert_not_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);
}
