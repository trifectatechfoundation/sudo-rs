//! Scenarios where a password does not need to be provided

use sudo_test::{As, EnvBuilder};

use crate::{Result, SUDOERS_ROOT_ALL};

// man sudoers > User Authentication:
// "A password is not required if the invoking user is root"
#[ignore]
#[test]
fn root_can_sudo_without_providing_a_password_if_root_user_is_in_sudoers_file() -> Result<()> {
    let env = EnvBuilder::default().sudoers(SUDOERS_ROOT_ALL).build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[ignore]
#[test]
fn root_can_sudo_without_providing_a_password_if_roots_group_is_in_sudoers_file() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers("%root    ALL=(ALL:ALL) ALL")
        .build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[test]
fn root_can_sudo_without_providing_a_password_if_roots_user_id_is_in_sudoers_file() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers("#0 ALL=(ALL:ALL) ALL")
        .build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[ignore]
#[test]
fn root_can_sudo_without_providing_a_password_if_roots_group_id_is_in_sudoers_file() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers("%#0 ALL=(ALL:ALL) ALL")
        .build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[ignore]
#[test]
fn user_can_sudo_without_providing_a_password_if_users_group_is_in_sudoers_file_and_nopasswd_is_set(
) -> Result<()> {
    let username = "ferris";
    let groupname = "rustaceans";
    let env = EnvBuilder::default()
        .sudoers(&format!("%{groupname}    ALL=(ALL:ALL) NOPASSWD: ALL"))
        .user(username, &[groupname])
        .build()?;

    let output = env.exec(&["sudo", "true"], As::User { name: username }, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[test]
fn user_can_sudo_without_providing_a_password_if_user_is_in_sudoers_file_and_nopasswd_is_set(
) -> Result<()> {
    let username = "ferris";
    let env = EnvBuilder::default()
        .sudoers(&format!("{username}    ALL=(ALL:ALL) NOPASSWD: ALL"))
        .user(username, &["rustaceans"])
        .build()?;

    let output = env.exec(&["sudo", "true"], As::User { name: username }, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

// man sudoers > User Authentication:
// "A password is not required if (..) the target user is the same as the invoking user"
#[ignore]
#[test]
fn user_can_become_themselves_without_providing_a_password() -> Result<()> {
    let username = "ferris";
    let env = EnvBuilder::default()
        .user(username, &[])
        .sudoers(&format!("{username}    ALL=(ALL:ALL) ALL"))
        .build()?;

    let output = env.exec(
        &["sudo", "-u", username, "true"],
        As::User { name: username },
        None,
    )?;

    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}
