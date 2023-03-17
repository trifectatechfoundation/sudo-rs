//! Test the first component of the user specification: `<user_list> ALL=(ALL:ALL) ALL`

use sudo_test::{As, EnvBuilder};

use crate::Result;

#[test]
fn no_match() -> Result<()> {
    let env = EnvBuilder::default().build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "root is not in the sudoers file");
    }

    Ok(())
}

#[test]
fn all() -> Result<()> {
    let username = "ferris";
    let env = EnvBuilder::default()
        .sudoers("ALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .user(username, &[])
        .build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    let output = env.exec(&["sudo", "true"], As::User { name: username }, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[test]
fn user_name() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers("root ALL=(ALL:ALL) NOPASSWD: ALL")
        .build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[test]
fn user_id() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers("#0 ALL=(ALL:ALL) NOPASSWD: ALL")
        .build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[test]
fn group_name() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers("%root ALL=(ALL:ALL) NOPASSWD: ALL")
        .build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[test]
fn group_id() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers("%#0 ALL=(ALL:ALL) NOPASSWD: ALL")
        .build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}
