use pretty_assertions::assert_eq;
use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

use super::MAX_PAM_RESPONSE_SIZE;

#[test]
fn correct_password() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?
        .assert_success()
}

#[test]
fn incorrect_password() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password("strong-password"))
        .build()?;

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin("incorrect-password")
        .output(&env)?;
    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "incorrect password attempt"
    } else {
        "incorrect authentication attempt"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn no_password() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .output(&env)?;
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "no password was provided"
    } else {
        "incorrect authentication attempt"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn longest_possible_password_works() -> Result<()> {
    let password = "a".repeat(MAX_PAM_RESPONSE_SIZE - 1 /* null byte */);

    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(&password))
        .build()?;

    Command::new("sudo")
        .args(["-S", "true"])
        .stdin(password)
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn input_longer_than_max_pam_response_size_is_handled_gracefully() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) ALL").user(USERNAME).build()?;

    let input = "a".repeat(5 * MAX_PAM_RESPONSE_SIZE / 2);
    let output = Command::new("sudo")
        .args(["-S", "true"])
        .stdin(input)
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_contains!(stderr, "sudo: 2 incorrect password attempts");
    } else {
        assert_contains!(stderr, "incorrect authentication attempt");
        assert_not_contains!(stderr, "panic");
    }

    Ok(())
}

#[test]
fn input_longer_than_password_should_not_be_accepted_as_correct_password() -> Result<()> {
    let password = "a".repeat(MAX_PAM_RESPONSE_SIZE - 1 /* null byte */);
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(password))
        .build()?;

    let input_sizes = [MAX_PAM_RESPONSE_SIZE, MAX_PAM_RESPONSE_SIZE + 1];

    for input_size in input_sizes {
        let input = "a".repeat(input_size);
        let output = Command::new("sudo")
            .args(["-S", "true"])
            .stdin(input)
            .as_user(USERNAME)
            .output(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        let stderr = output.stderr();
        if sudo_test::is_original_sudo() {
            assert_contains!(stderr, "sudo: 1 incorrect password attempt");
        } else {
            assert_contains!(stderr, "incorrect authentication attempt");
        }
    }

    Ok(())
}
