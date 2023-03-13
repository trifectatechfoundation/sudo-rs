use pretty_assertions::assert_eq;
use sudo_test::{As, EnvBuilder};

use crate::{Result, SUDOERS_FERRIS_ALL_NOPASSWD, SUDOERS_ROOT_ALL};

#[ignore]
#[test]
fn root_can_become_another_user() -> Result<()> {
    let env = EnvBuilder::default()
        .user("ferris", &[])
        .sudoers(SUDOERS_ROOT_ALL)
        .build()?;

    let expected = env.stdout(&["id"], As::User { name: "ferris" }, None)?;
    let actual = env.stdout(&["sudo", "-u", "ferris", "id"], As::Root, None)?;

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
