//! Scenarios where password authentication is needed

use pretty_assertions::assert_eq;

use sudo_test::{As, EnvBuilder};

use crate::Result;

#[ignore]
#[test]
fn user_can_sudo_if_users_group_is_in_sudoers_file_and_correct_password_is_provided() -> Result<()>
{
    let username = "ferris";
    let groupname = "rustaceans";
    let password = "strong-password";
    let env = EnvBuilder::default()
        .sudoers(&format!("%{groupname}    ALL=(ALL:ALL) ALL"))
        .user(username, &[groupname])
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
fn user_cannot_sudo_if_users_group_is_in_sudoers_file_and_incorrect_password_is_provided(
) -> Result<()> {
    let username = "ferris";
    let groupname = "rustaceans";
    let env = EnvBuilder::default()
        .sudoers(&format!("%{groupname}    ALL=(ALL:ALL) ALL"))
        .user(username, &[groupname])
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
fn user_cannot_sudo_if_users_group_is_in_sudoers_file_and_password_is_not_provided() -> Result<()> {
    let username = "ferris";
    let groupname = "rustaceans";
    let password = "strong-password";
    let env = EnvBuilder::default()
        .sudoers(&format!("%{groupname}    ALL=(ALL:ALL) ALL"))
        .user(username, &[groupname])
        .user_password(username, password)
        .build()?;

    let output = env.exec(&["sudo", "-S", "true"], As::User { name: username }, None)?;
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "no password was provided");
    }

    Ok(())
}

#[ignore]
#[test]
fn user_can_sudo_if_user_is_in_sudoers_file_and_correct_password_is_provided() -> Result<()> {
    let username = "ferris";
    let password = "strong-password";
    let env = EnvBuilder::default()
        .sudoers(&format!("{username}    ALL=(ALL:ALL) ALL"))
        .user(username, &["rustaceans"])
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
fn user_cannot_sudo_if_user_is_in_sudoers_file_and_incorrect_password_is_provided() -> Result<()> {
    let username = "ferris";
    let env = EnvBuilder::default()
        .sudoers(&format!("{username}    ALL=(ALL:ALL) ALL"))
        .user(username, &["rustaceans"])
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
fn user_cannot_sudo_if_user_is_in_sudoers_file_and_password_is_not_provided() -> Result<()> {
    let username = "ferris";
    let password = "strong-password";
    let env = EnvBuilder::default()
        .sudoers(&format!("{username}    ALL=(ALL:ALL) ALL"))
        .user(username, &["rustaceans"])
        .user_password(username, password)
        .build()?;

    let output = env.exec(&["sudo", "-S", "true"], As::User { name: username }, None)?;
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "no password was provided");
    }

    Ok(())
}

#[ignore]
#[test]
fn user_can_sudo_if_user_id_is_in_sudoers_file_and_correct_password_is_provided() -> Result<()> {
    let username = "ferris";
    let user_id = 1023;
    let password = "strong-password";
    let env = EnvBuilder::default()
        .sudoers(&format!("#{user_id}    ALL=(ALL:ALL) ALL"))
        .user(username, &[])
        .user_id(username, user_id)
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

#[ignore]
#[test]
fn user_can_sudo_if_theirs_group_id_is_in_sudoers_file_and_correct_password_is_provided(
) -> Result<()> {
    let username = "ferris";
    let group_id = 1023;
    let groupname = "rustaceans";
    let password = "strong-password";
    let env = EnvBuilder::default()
        .sudoers(&format!("%#{group_id}    ALL=(ALL:ALL) ALL"))
        .user(username, &[groupname])
        .group_id(groupname, group_id)
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
