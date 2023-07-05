//! PAM integration tests

use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

#[test]
fn given_pam_permit_then_no_password_auth_required() -> Result<()> {
    let env = Env("")
        .user(USERNAME)
        .file("/etc/pam.d/su", "auth sufficient pam_permit.so")
        .build()?;

    Command::new("su")
        .args(["-c", "/usr/bin/true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn given_pam_deny_then_password_auth_always_fails() -> Result<()> {
    let invoking_user = USERNAME;
    let target_user = "ghost";

    let env = Env("")
        .file("/etc/pam.d/su", "auth requisite pam_deny.so")
        .user(invoking_user)
        .user(User(target_user).password(PASSWORD))
        .build()?;

    let output = Command::new("su")
        .args(["-s", "/usr/bin/true", target_user])
        .as_user(invoking_user)
        .stdin(PASSWORD)
        .output(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());
    
        let diagnostic = if sudo_test::is_original_sudo() {
            "su: Authentication failure"
        } else {
            "3 incorrect authentication attempts"
        };
        assert_contains!(output.stderr(), diagnostic);

    Ok(())
}


#[test]
fn being_root_has_precedence_over_missing_pam_file() -> Result<()> {
    let env = Env("")
        .build()?;

    Command::new("su")
        .output(&env)?
        .assert_success()
}

#[test]
fn being_root_has_no_precedence_over_pam_deny() -> Result<()> {
    let env = Env("")
        .file("/etc/pam.d/su", "auth requisite pam_deny.so")
        .build()?;

    let output = Command::new("su")
        .args(["-c", "/usr/bin/true"])
        .output(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());
    
        let diagnostic = if sudo_test::is_original_sudo() {
            "su: Authentication failure"
        } else {
            "3 incorrect authentication attempts"
        };
        assert_contains!(output.stderr(), diagnostic);

    Ok(())
}
