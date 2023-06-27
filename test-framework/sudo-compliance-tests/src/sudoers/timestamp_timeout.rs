use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

#[test]
fn credential_caching_works_with_custom_timeout() -> Result<()> {
    let env = Env(format!(
        "{USERNAME} ALL=(ALL:ALL) ALL
Defaults timestamp_timeout=0.1"
    ))
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    // input valid credentials
    // try to sudo without a password
    Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S true; sudo true"))
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn nonzero() -> Result<()> {
    let env = Env(format!(
        "{USERNAME} ALL=(ALL:ALL) ALL
Defaults timestamp_timeout=0.1"
    ))
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    // input valid credentials
    // wait until they expire / timeout
    // try to sudo without a password
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sleep 10; sudo true"
        ))
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "a password is required"
    } else {
        "incorrect authentication attempt"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn zero_always_prompts_for_password() -> Result<()> {
    let env = Env(format!(
        "{USERNAME} ALL=(ALL:ALL) ALL
Defaults timestamp_timeout=0"
    ))
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    // input valid credentials
    // try to sudo without a password
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S true; sudo true"))
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "a password is required"
    } else {
        "incorrect authentication attempt"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}
