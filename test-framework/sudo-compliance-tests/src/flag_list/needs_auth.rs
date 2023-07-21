use sudo_test::{Command, Env};

use crate::{Result, USERNAME};

#[test]
fn when_other_user_is_self() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) ALL").user(USERNAME).build()?;

    let output = Command::new("sudo")
        .args(["-S", "-l", "-U", USERNAME])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("[sudo] password for {USERNAME}:")
    } else {
        "[sudo: authenticate] Password:".to_string()
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn other_user_has_nopasswd_tag() -> Result<()> {
    let other_user = "ghost";
    let env = Env(format!(
        "{other_user} ALL=(ALL:ALL) NOPASSWD: ALL
{USERNAME} ALL=(ALL:ALL) ALL"
    ))
    .user(USERNAME)
    .user(other_user)
    .build()?;

    let output = Command::new("sudo")
        .args(["-S", "-l", "-U", other_user])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("[sudo] password for {USERNAME}:")
    } else {
        "[sudo: authenticate] Password:".to_string()
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}
