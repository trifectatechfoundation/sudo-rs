//! Scenarios where a password does not need to be provided

use sudo_test::{As, EnvBuilder};

use crate::{Result, SUDOERS_ROOT_ALL};

// NOTE all these tests assume that the invoking user passes the sudoers file 'User_List' criteria

// man sudoers > User Authentication:
// "A password is not required if the invoking user is root"
#[ignore]
#[test]
fn user_is_root() -> Result<()> {
    let env = EnvBuilder::default().sudoers(SUDOERS_ROOT_ALL).build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

// man sudoers > User Authentication:
// "A password is not required if (..) the target user is the same as the invoking user"
#[ignore]
#[test]
fn user_as_themselves() -> Result<()> {
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

#[test]
fn nopasswd_tag() -> Result<()> {
    let username = "ferris";
    let env = EnvBuilder::default()
        .sudoers(&format!("{username}    ALL=(ALL:ALL) NOPASSWD: ALL"))
        .user(username, &[])
        .build()?;

    let output = env.exec(&["sudo", "true"], As::User { name: username }, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}
