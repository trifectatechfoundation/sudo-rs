use sudo_test::{Command, Env, User};

use crate::{Result, PANIC_EXIT_CODE, PASSWORD, USERNAME};

mod cli;
mod env;
mod flag_command;
mod flag_group;
mod flag_login;
mod flag_preserve_environment;
mod flag_shell;
mod flag_supp_group;
mod flag_whitelist_environment;
mod limits;
mod pam;
mod syslog;

#[test]
fn default_target_is_root() {
    let env = Env("").build();

    let actual = Command::new("su")
        .args(["-c", "whoami"])
        .output(&env)
        .stdout();

    let expected = "root";
    assert_eq!(expected, actual);
}

#[test]
fn explicit_target_user() {
    let env = Env("").user(USERNAME).build();

    let actual = Command::new("su")
        .args(["-c", "whoami", USERNAME])
        .output(&env)
        .stdout();

    assert_eq!(USERNAME, actual);
}

#[test]
fn target_user_must_exist_in_passwd_db() {
    let env = Env("").build();

    let output = Command::new("su")
        .args([USERNAME, "-c", "true"])
        .output(&env);

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("user {USERNAME} does not exist or the user entry does not contain all the required fields")
    } else {
        format!("user '{USERNAME}' not found")
    };

    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn required_password_is_target_users_pass() {
    let invoking_user = "ferris";
    let target_user_name = "ghost";
    let target_user_password = PASSWORD;
    let env = Env("")
        .user(invoking_user)
        .user(User(target_user_name).password(target_user_password))
        .build();

    let actual = Command::new("su")
        .args([target_user_name, "-c", "whoami"])
        .stdin(target_user_password)
        .as_user(invoking_user)
        .output(&env)
        .stdout();

    assert_eq!(target_user_name, actual);
}

#[test]
fn required_password_is_target_users_fail() {
    let target_user = "ghost";
    let env = Env("")
        .user(User(USERNAME).password(PASSWORD))
        .user(target_user)
        .build();

    let output = Command::new("su")
        .args([target_user, "-c", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env);

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "Authentication failure"
    } else {
        "Maximum 3 incorrect authentication attempts"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn nopasswd_root() {
    let env = Env("").user(USERNAME).build();

    Command::new("su")
        .args([USERNAME, "-c", "true"])
        .output(&env)
        .assert_success();
}

#[test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "su on FreeBSD doesn't require password if target user is self"
)]
fn password_is_required_when_target_user_is_self() {
    let env = Env("").user(USERNAME).build();

    let output = Command::new("su")
        .args([USERNAME, "-c", "true"])
        .as_user(USERNAME)
        .output(&env);

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "Authentication failure"
    } else {
        "Maximum 3 incorrect authentication attempts"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn does_not_panic_on_io_errors() -> Result<()> {
    let env = Env("").build();

    let output = Command::new("bash")
        .args(["-c", "su --help | true; echo \"${PIPESTATUS[0]}\""])
        .output(&env);

    let stderr = output.stderr();
    assert!(stderr.is_empty());

    let exit_code = output.stdout().parse()?;
    assert_ne!(PANIC_EXIT_CODE, exit_code);
    // ogsu exits with 141 = SIGPIPE; su-rs exits with code 1 but the difference is not
    // relevant to this test
    // assert_eq!(141, exit_code);

    Ok(())
}
