use pretty_assertions::assert_eq;
use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_FERRIS_ALL_NOPASSWD, SUDOERS_ROOT_ALL_NOPASSWD};

#[test]
fn root_can_become_another_user_by_name() -> Result<()> {
    let username = "ferris";
    let env = Env::new(SUDOERS_ROOT_ALL_NOPASSWD)
        .user(username, &[])
        .build()?;

    let expected = Command::new("id").as_user(username).exec(&env)?.stdout()?;
    let actual = Command::new("sudo")
        .args(["-u", username, "id"])
        .exec(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn root_can_become_another_user_by_uid() -> Result<()> {
    let username = "ferris";
    let env = Env::new(SUDOERS_ROOT_ALL_NOPASSWD)
        .user(username, &[])
        .build()?;

    let uid = Command::new("id")
        .arg("-u")
        .as_user(username)
        .exec(&env)?
        .stdout()?
        .parse::<u32>()?;
    let expected = Command::new("id").as_user(username).exec(&env)?.stdout()?;
    let actual = Command::new("sudo")
        .arg("-u")
        .arg(format!("#{uid}"))
        .arg("id")
        .exec(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[ignore]
#[test]
fn user_can_become_another_user() -> Result<()> {
    let env = Env::new(SUDOERS_FERRIS_ALL_NOPASSWD)
        .user("ferris", &[])
        .user("someone_else", &[])
        .build()?;

    let expected = Command::new("id")
        .as_user("someone_else")
        .exec(&env)?
        .stdout()?;
    let actual = Command::new("sudo")
        .args(["-u", "someone_else", "id"])
        .as_user("ferris")
        .exec(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}
