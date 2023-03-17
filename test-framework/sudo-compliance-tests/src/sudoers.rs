use sudo_test::{As, EnvBuilder};

use crate::Result;

#[test]
fn cannot_sudo_with_empty_sudoers_file() -> Result<()> {
    let env = EnvBuilder::default().build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "root is not in the sudoers file");
    }

    Ok(())
}

#[test]
fn cannot_sudo_if_sudoers_file_is_world_writable() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers("ALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .sudoers_chmod("446")
        .build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "/etc/sudoers is world writable");
    }

    Ok(())
}

#[ignore]
#[test]
fn user_specifications_evaluated_bottom_to_top() -> Result<()> {
    let username = "ferris";
    let password = "strong-password";
    let env = EnvBuilder::default()
        .user(username, &[])
        .user_password(username, password)
        .sudoers("ferris ALL=(ALL:ALL) NOPASSWD: ALL")
        // overrides the preceding NOPASSWD
        .sudoers("ferris ALL=(ALL:ALL) ALL")
        .build()?;

    let output = env.exec(&["sudo", "-S", "true"], As::User { name: username }, None)?;
    assert!(!output.status.success());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "no password was provided");
    }

    let output = env.exec(
        &["sudo", "-S", "true"],
        As::User { name: username },
        Some(password),
    )?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}
