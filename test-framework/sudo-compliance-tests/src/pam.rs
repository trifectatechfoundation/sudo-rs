//! PAM integration tests

use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

mod env;

#[test]
fn given_pam_permit_then_no_password_auth_required() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(USERNAME)
        .file("/etc/pam.d/sudo", "auth sufficient pam_permit.so")
        .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}

#[test]
fn given_pam_deny_then_password_auth_always_fails() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(PASSWORD))
        .file("/etc/pam.d/sudo", "auth requisite pam_deny.so")
        .build()?;

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "3 incorrect password attempts"
    } else {
        "3 incorrect authentication attempts"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn being_root_has_precedence_over_pam() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file("/etc/pam.d/sudo", "auth requisite pam_deny.so")
        .build()?;

    Command::new("sudo")
        .args(["true"])
        .exec(&env)?
        .assert_success()
}

#[test]
fn nopasswd_in_sudoers_has_precedence_over_pam() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .file("/etc/pam.d/sudo", "auth requisite pam_deny.so")
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}
