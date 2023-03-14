//! Scenarios where password authentication is needed

// NOTE all these tests assume that the invoking user passes the sudoers file 'User_List' criteria

use pretty_assertions::assert_eq;

use sudo_test::{As, EnvBuilder};

use crate::Result;

#[ignore]
#[test]
fn correct_password() -> Result<()> {
    let username = "ferris";
    let password = "strong-password";
    let env = EnvBuilder::default()
        .sudoers(&format!("{username}    ALL=(ALL:ALL) ALL"))
        .user(username, &[])
        .user_password(username, password)
        .build()?;

    let output = env.exec(
        &["sudo", "-S", "true"],
        As::User { name: username },
        Some(password),
    )?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[test]
fn incorrect_password() -> Result<()> {
    let username = "ferris";
    let env = EnvBuilder::default()
        .sudoers(&format!("{username}    ALL=(ALL:ALL) ALL"))
        .user(username, &[])
        .user_password(username, "strong-password")
        .build()?;

    let output = env.exec(
        &["sudo", "-S", "true"],
        As::User { name: username },
        Some("incorrect-password"),
    )?;
    assert!(!output.status.success());
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "incorrect password attempt");
    }

    Ok(())
}

#[test]
fn no_password() -> Result<()> {
    let username = "ferris";
    let password = "strong-password";
    let env = EnvBuilder::default()
        .sudoers(&format!("{username}    ALL=(ALL:ALL) ALL"))
        .user(username, &[])
        .user_password(username, password)
        .build()?;

    let output = env.exec(&["sudo", "-S", "true"], As::User { name: username }, None)?;
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "no password was provided");
    }

    Ok(())
}
