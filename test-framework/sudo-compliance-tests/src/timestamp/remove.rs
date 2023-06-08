use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

#[test]
fn is_limited_to_a_single_user() -> Result<()> {
    let second_user = "ghost";
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(PASSWORD))
        .user(User(second_user).password(PASSWORD))
        .build()?;

    let child = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; touch /tmp/barrier1; until [ -f /tmp/barrier2 ]; do sleep 1; done; sudo -S true"
        ))
        .as_user(USERNAME)
        .spawn(&env)?;

    Command::new("sh")
        .arg("-c")
        .arg("until [ -f /tmp/barrier1 ]; do sleep 1; done; sudo -K && touch /tmp/barrier2")
        .as_user(second_user)
        .exec(&env)?
        .assert_success()?;

    child.wait()?.assert_success()
}

#[test]
fn has_a_user_global_effect() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let child = Command::new("sh")
    .arg("-c")
    .arg(format!(
        "echo {PASSWORD} | sudo -S true; touch /tmp/barrier1; until [ -f /tmp/barrier2 ]; do sleep 1; done; echo | sudo -S true"
    ))
    .as_user(USERNAME)
    .spawn(&env)?;

    Command::new("sh")
        .arg("-c")
        .arg("until [ -f /tmp/barrier1 ]; do sleep 1; done; sudo -K && touch /tmp/barrier2")
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()?;

    let output = child.wait()?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "1 incorrect password attempt"
    } else {
        "incorrect authentication attempt"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn also_works_locally() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    // input valid credentials
    // invalidate them
    // try to sudo without a password
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -K; sudo true"
        ))
        .as_user(USERNAME)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "a password is required"
    } else {
        "Authentication failed"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}
