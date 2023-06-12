use crate::{
    Result, OG_SUDO_STANDARD_LECTURE, PASSWORD, SUDOERS_ALWAYS_LECTURE, SUDOERS_NO_LECTURE,
    SUDOERS_ONCE_LECTURE, SUDOERS_ROOT_ALL, SUDOERS_USER_ALL_ALL, SUDOERS_USER_ALL_NOPASSWD,
    USERNAME,
};
use sudo_test::{Command, Env, User};

#[test]
#[ignore = "gh399"]
fn default_lecture_shown_once() -> Result<()> {
    let env = Env([SUDOERS_ROOT_ALL, SUDOERS_ONCE_LECTURE, SUDOERS_USER_ALL_ALL])
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .exec(&env)?;
    assert!(output.status().success());

    assert_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);

    let second_sudo = Command::new("sudo")
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .args(["-S", "echo", "Yeah!"])
        .exec(&env)?;

    assert!(second_sudo.status().success());
    assert_not_contains!(second_sudo.stderr(), OG_SUDO_STANDARD_LECTURE);
    Ok(())
}

#[test]
#[ignore = "gh399"]
fn lecture_in_stderr() -> Result<()> {
    let env = Env([SUDOERS_ROOT_ALL, SUDOERS_ONCE_LECTURE, SUDOERS_USER_ALL_ALL])
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .exec(&env)?;
    assert!(output.status().success());

    assert_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);

    assert_not_contains!(output.stdout().unwrap(), OG_SUDO_STANDARD_LECTURE);
    Ok(())
}

#[test]
#[ignore = "gh399"]
fn lecture_always_shown() -> Result<()> {
    let env = Env([
        SUDOERS_ROOT_ALL,
        SUDOERS_ONCE_LECTURE,
        SUDOERS_ALWAYS_LECTURE,
    ])
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    let output = Command::new("sudo")
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .args(["-S", "true"])
        .exec(&env)?;
    assert!(!output.status().success());

    assert_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);

    let second_sudo = Command::new("sudo")
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .args(["-S", "ls"])
        .exec(&env)?;
    assert!(!output.status().success());

    assert_contains!(second_sudo.stderr(), OG_SUDO_STANDARD_LECTURE);
    Ok(())
}

#[test]
fn lecture_never_shown() -> Result<()> {
    let env = Env([SUDOERS_ROOT_ALL, SUDOERS_USER_ALL_ALL, SUDOERS_NO_LECTURE])
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .args(["-S", "echo", "Yeah!"])
        .exec(&env)?;

    assert!(output.status().success());
    assert_not_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);
    Ok(())
}

#[test]
fn negation_equals_never() -> Result<()> {
    let env = Env([SUDOERS_ROOT_ALL, SUDOERS_USER_ALL_ALL, "Defaults  !lecture"])
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .args(["-S", "echo", "Yeah!"])
        .exec(&env)?;

    assert!(output.status().success());
    assert_not_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);
    Ok(())
}

#[test]
fn double_negation_also_equals_never() -> Result<()> {
    let env = Env([
        SUDOERS_ROOT_ALL,
        SUDOERS_USER_ALL_ALL,
        "Defaults  !!lecture",
    ])
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .exec(&env)?;

    assert!(output.status().success());
    assert_not_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);

    Ok(())
}

/// Lectures are only shown when password is asked for
#[test]
fn root_user_lecture_not_shown() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL).build()?;

    let output = Command::new("sudo")
        .as_user("root")
        .stdin(PASSWORD)
        .args(["-S", "true"])
        .exec(&env)?;

    assert!(output.status().success());
    assert_not_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);

    Ok(())
}

#[test]
fn nopasswd_lecture_not_shown() -> Result<()> {
    let env = Env(SUDOERS_USER_ALL_NOPASSWD)
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .args(["-S", "true"])
        .exec(&env)?;

    assert!(output.status().success());
    assert_not_contains!(output.stderr(), OG_SUDO_STANDARD_LECTURE);

    Ok(())
}
