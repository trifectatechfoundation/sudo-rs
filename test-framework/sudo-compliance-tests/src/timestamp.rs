use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

mod remove;
mod reset;
mod validate;

#[test]
fn credential_caching_works() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!("set -e; echo {PASSWORD} | sudo -S true; sudo true"))
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn by_default_credential_caching_is_local() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!("set -e; echo {PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
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
fn credential_cache_is_shared_with_child_shell() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; echo {PASSWORD} | sudo -S true; sh -c 'sudo true'"
        ))
        .as_user(USERNAME)
        // XXX unclear why this and the tests that follow need a pseudo-TTY allocation to pass
        .tty(true)
        .output(&env)?
        .assert_success()
}

#[test]
fn credential_cache_is_shared_with_parent_shell() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; sh -c 'echo {PASSWORD} | sudo -S true'; sudo true"
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)?
        .assert_success()
}

#[test]
fn credential_cache_is_shared_between_sibling_shells() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; sh -c 'echo {PASSWORD} | sudo -S true'; sh -c 'sudo true'"
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)?
        .assert_success()
}

#[test]
fn cached_credential_applies_to_all_target_users() -> Result<()> {
    let second_target_user = "ghost";
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .user(second_target_user)
        .build()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; echo {PASSWORD} | sudo -S true; sudo -u {second_target_user} true"
        ))
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn cached_credential_not_shared_with_target_user_that_are_not_self() -> Result<()> {
    let second_target_user = "ghost";
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(PASSWORD))
        .user(second_target_user)
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -u {second_target_user} -S true; sudo -u {second_target_user} sudo -S true"
        ))
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());

    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "a password is required"
    } else {
        "Maximum 3 incorrect authentication attempts"
    };

    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn cached_credential_shared_with_target_user_that_is_self_on_the_same_tty() -> Result<()> {
    let env = Env(["Defaults !use_pty".to_string(), format!("{USERNAME} ALL=(ALL:ALL) ALL")])
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -u {USERNAME} sudo -n true"
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn cached_credential_not_shared_with_self_across_ttys() -> Result<()> {
    let env = Env(["Defaults use_pty".to_string(), format!("{USERNAME} ALL=(ALL:ALL) ALL")])
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -u {USERNAME} sudo -n true"
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    Ok(())
}
