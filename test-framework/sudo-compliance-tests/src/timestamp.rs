use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

#[test]
fn credential_caching_works() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!("set -e; echo {PASSWORD} | sudo -S true; sudo true"))
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}

#[test]
#[ignore]
fn sudoers_defaults_timestamp_timeout_nonzero() -> Result<()> {
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
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "a password is required");
    }

    Ok(())
}

#[test]
#[ignore]
fn sudoers_defaults_timestamp_timeout_zero_always_prompts_for_password() -> Result<()> {
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
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "a password is required");
    }

    Ok(())
}

#[test]
#[ignore]
fn flag_reset_timsetamp() -> Result<()> {
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

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "a password is required");
    }

    Ok(())
}

#[test]
#[ignore]
fn flag_validate_revalidation() -> Result<()> {
    let env = Env(format!(
        "{USERNAME} ALL=(ALL:ALL) ALL
Defaults timestamp_timeout=0.1"
    ))
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    // input valid credentials
    // revalidate credentials a few times
    // sudo without a password, using re-validated credentials
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; echo {PASSWORD} | sudo -S true; for i in {{1..5}}; do sleep 3; sudo -v; done; sudo true"
        ))
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}

#[test]
#[ignore]
fn flag_validate_prompts_for_password() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
        .arg("-v")
        .as_user(USERNAME)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "a password is required");
    }

    Ok(())
}
