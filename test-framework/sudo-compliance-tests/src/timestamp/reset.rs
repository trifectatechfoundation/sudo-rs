use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

#[test]
fn it_works() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    // input valid credentials
    // invalidate them
    // try to sudo without a password
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -k; sudo true"
        ))
        .as_user(USERNAME)
        .exec(&env)?;

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
fn has_a_local_effect() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let child = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; echo {PASSWORD} | sudo -S true; touch /tmp/barrier1; until [ -f /tmp/barrier2 ]; do sleep 1; done; sudo true"
        ))
        .as_user(USERNAME)
        .spawn(&env)?;

    Command::new("sh")
        .arg("-c")
        .arg("until [ -f /tmp/barrier1 ]; do sleep 1; done; sudo -k; touch /tmp/barrier2")
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()?;

    child.wait()?.assert_success()
}

#[test]
fn with_command_prompts_for_password() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S true; sudo -k true"))
        .as_user(USERNAME)
        .exec(&env)?;

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
fn with_command_failure_does_not_invalidate_credential_cache() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    // the first command, `sudo -S true`, succeeds and caches credentials
    //
    // the second command, `sudo -k true`, prompts for a password and fails because no password is
    // provided
    //
    // the last command, `sudo true`, is expected to work because `sudo -k true` did *not*
    // invalidate the credentials
    //
    // the `[ $? -eq 1 ] || exit 2` bit is there to turn the success of `sudo -k true` (which is
    // expected to fail) into a failure of the entire shell script
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -k true; [ $? -eq 1 ] || exit 2; sudo true"
        ))
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}

#[test]
fn with_command_success_does_not_invalidate_credential_cache() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    // the first command, `sudo -S true`, succeeds and caches credentials
    //
    // the second command, `sudo -k true`, succeeds
    //
    // the last command, `sudo true`, is expected to work because `sudo -k true` did *not*
    // invalidate the credentials
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; echo {PASSWORD} | sudo -k -S true; sudo true"
        ))
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}

#[test]
fn with_command_does_not_cache_credentials() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -k -S true 2>/dev/null || exit 2; sudo true"
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
