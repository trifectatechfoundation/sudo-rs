use pretty_assertions::assert_eq;
use sudo_test::{As, EnvBuilder};

use crate::{Result, SUDOERS_FERRIS_ALL_NOPASSWD, SUDOERS_ROOT_ALL};

#[ignore]
#[test]
fn root_can_become_another_user_by_name() -> Result<()> {
    let username = "ferris";
    let env = EnvBuilder::default()
        .user(username, &[])
        .sudoers(SUDOERS_ROOT_ALL)
        .build()?;

    let expected = env.stdout(&["id"], As::User { name: username }, None)?;
    let actual = env.stdout(&["sudo", "-u", username, "id"], As::Root, None)?;

    assert_eq!(expected, actual);

    Ok(())
}

#[ignore]
#[test]
fn root_can_become_another_user_by_uid() -> Result<()> {
    let username = "ferris";
    let env = EnvBuilder::default()
        .user(username, &[])
        .sudoers(SUDOERS_ROOT_ALL)
        .build()?;

    let uid = env
        .stdout(&["id", "-u"], As::User { name: username }, None)?
        .parse::<u32>()?;
    let expected = env.stdout(&["id"], As::User { name: username }, None)?;
    let actual = env.stdout(&["sudo", "-u", &format!("#{uid}"), "id"], As::Root, None)?;

    assert_eq!(expected, actual);

    Ok(())
}

#[ignore]
#[test]
fn user_can_become_another_user() -> Result<()> {
    let env = EnvBuilder::default()
        .user("ferris", &[])
        .user("someone_else", &[])
        .sudoers(SUDOERS_FERRIS_ALL_NOPASSWD)
        .build()?;

    let expected = env.stdout(
        &["id"],
        As::User {
            name: "someone_else",
        },
        None,
    )?;
    let actual = env.stdout(
        &["sudo", "-u", "someone_else", "id"],
        As::User { name: "ferris" },
        None,
    )?;

    assert_eq!(expected, actual);

    Ok(())
}
