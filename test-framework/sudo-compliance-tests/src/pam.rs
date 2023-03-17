//! PAM integration tests

use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

#[test]
fn pam_permit() -> Result<()> {
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
fn pam_deny() -> Result<()> {
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

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "3 incorrect password attempts");
    }

    Ok(())
}

#[test]
#[ignore]
fn being_root_has_precedence() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file("/etc/pam.d/sudo", "auth requisite pam_deny.so")
        .build()?;

    Command::new("sudo")
        .args(["true"])
        .exec(&env)?
        .assert_success()
}

#[test]
fn nopasswd_has_precedence() -> Result<()> {
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
