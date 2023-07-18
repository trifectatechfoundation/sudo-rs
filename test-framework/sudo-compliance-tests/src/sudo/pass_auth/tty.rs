use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

use super::MAX_PAM_RESPONSE_SIZE;

#[test]
#[ignore = "gh414"]
fn correct_password() -> Result<()> {
    if !sudo_test::is_original_sudo() {
        return Err("FIXME flaky test".into());
    }

    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sshpass")
        .args(["-p", PASSWORD, "sudo", "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn incorrect_password() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password("strong-password"))
        .build()?;

    let output = Command::new("sshpass")
        .args(["-p", "incorrect-password", "sudo", "true"])
        .as_user(USERNAME)
        .output(&env)?;
    assert!(!output.status().success());

    // `sshpass` will override sudo's exit code with the value 5 so we can't check this
    // assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "1 incorrect password attempt");
    }

    Ok(())
}

#[test]
fn no_tty() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
        .args(["true"])
        .as_user(USERNAME)
        .output(&env)?;
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "a terminal is required to read the password"
    } else {
        "Maximum 3 incorrect authentication attempts"
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

    Command::new("sshpass")
        .args(["-p", &password, "sudo", "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
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
        let output = Command::new("sshpass")
            .args(["-p", &input, "sudo", "true"])
            .as_user(USERNAME)
            .output(&env)?;

        assert!(!output.status().success());
        // `sshpass` will override sudo's exit code with the value 5 so we can't check this
        // assert_eq!(Some(1), output.status().code());

        let stderr = output.stderr();
        let diagnostic = if sudo_test::is_original_sudo() {
            "sudo: 1 incorrect password attempt"
        } else {
            "Authentication failed, try again"
        };
        assert_contains!(stderr, diagnostic);
    }

    Ok(())
}
