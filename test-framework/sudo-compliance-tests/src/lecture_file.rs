use crate::{
    Result, OG_SUDO_STANDARD_LECTURE, PASSWORD, SUDOERS_NEW_LECTURE, SUDOERS_NEW_LECTURE_USER,
    SUDOERS_ONCE_LECTURE, SUDOERS_ROOT_ALL, USERNAME,
};
use sudo_test::{Command, Env, User};

#[ignore = "gh399"]
#[test]
fn default_lecture_message() -> Result<()> {
    let env = Env([SUDOERS_ROOT_ALL, SUDOERS_ONCE_LECTURE])
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?;

    assert_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);
    Ok(())
}

#[ignore = "gh400"]
#[test]
fn new_lecture_message() -> Result<()> {
    let new_lecture = "I <3 sudo";
    let env = Env([SUDOERS_ROOT_ALL, SUDOERS_ONCE_LECTURE, SUDOERS_NEW_LECTURE])
        .file("/etc/sudo_lecture", new_lecture)
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .args(["-S", "true"])
        .output(&env)?;
    assert!(!output.status().success());
    assert_contains!(output.stderr(), "I <3 sudo");
    Ok(())
}

#[test]
#[ignore = "gh400"]
fn new_lecture_for_specific_user() -> Result<()> {
    let new_lecture = "I <3 sudo";
    let env = Env([
        SUDOERS_ROOT_ALL,
        SUDOERS_ONCE_LECTURE,
        SUDOERS_NEW_LECTURE_USER,
    ])
    .file("/etc/sudo_lecture", new_lecture)
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    let output = Command::new("sudo")
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .args(["-S", "true"])
        .output(&env)?;
    assert!(!output.status().success());
    assert_contains!(output.stderr(), "I <3 sudo");
    Ok(())
}

#[ignore = "gh400"]
#[test]
fn default_lecture_for_unspecified_user() -> Result<()> {
    let new_lecture = "I <3 sudo";
    let env = Env([
        SUDOERS_ROOT_ALL,
        SUDOERS_ONCE_LECTURE,
        SUDOERS_NEW_LECTURE_USER,
    ])
    .file("/etc/sudo_lecture", new_lecture)
    .user(User(USERNAME).password(PASSWORD))
    .user(User("other_user").password("other_password"))
    .build()?;

    let output = Command::new("sudo")
        .as_user("other_user")
        .stdin("other_password")
        .args(["-S", "true"])
        .output(&env)?;
    assert!(!output.status().success());
    assert_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);
    Ok(())
}
