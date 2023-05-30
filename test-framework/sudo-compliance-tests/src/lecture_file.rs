use sudo_test::{Command, Env, User};
use crate::{Result, SUDOERS_ROOT_ALL, USERNAME, PASSWORD, SUDOERS_NEW_LECTURE, SUDOERS_NEW_LECTURE_USER, OG_SUDO_STANDARD_LECTURE};

#[ignore]
#[test]
fn default_lecture_message() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL)
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
    .args(["-S", "true"])
    .as_user(USERNAME)
    .stdin(PASSWORD)
    .exec(&env)?;

    assert_contains!(
        output.stderr(),
        OG_SUDO_STANDARD_LECTURE
    );
    Ok(())
}

#[ignore]
#[test]
fn new_lecture_message() -> Result<()> {
    let new_lecture = format!("I <3 sudo");
    let env = Env([
        SUDOERS_ROOT_ALL,
        SUDOERS_NEW_LECTURE])
        .file("/etc/sudo_lecture", new_lecture)
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
    .as_user(USERNAME)
    .stdin(PASSWORD)
    .args(["-S", "true"])
    .exec(&env)?;
    assert_eq!(false, output.status().success());
    assert_contains!(
        output.stderr(),
        "I <3 sudo"
    );
    Ok(())
}

#[test]
#[ignore]
fn new_lecture_for_specific_user() -> Result<()> {
    let new_lecture = format!("I <3 sudo");
    let env = Env([
        SUDOERS_ROOT_ALL,
        SUDOERS_NEW_LECTURE_USER])
        .file("/etc/sudo_lecture", new_lecture)
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
    .as_user(USERNAME)
    .stdin(PASSWORD)
    .args(["-S", "true"])
    .exec(&env)?;
    assert_eq!(false, output.status().success());
    assert_contains!(
        output.stderr(),
        "I <3 sudo"
    );
    Ok(())
}

#[ignore]
#[test]
fn default_lecture_for_unspecified_user() -> Result<()> {
    let new_lecture = format!("I <3 sudo");
    let env = Env([
        SUDOERS_ROOT_ALL,
        SUDOERS_NEW_LECTURE_USER])
        .file("/etc/sudo_lecture", new_lecture)
        .user(User(USERNAME).password(PASSWORD))
        .user(User("other_user").password("other_password"))
        .build()?;

    let output = Command::new("sudo")
    .as_user("other_user")
    .stdin("other_password")
    .args(["-S", "true"])
    .exec(&env)?;
    assert_eq!(false, output.status().success());
    assert_contains!(
        output.stderr(),
        OG_SUDO_STANDARD_LECTURE
    );
    Ok(())
}