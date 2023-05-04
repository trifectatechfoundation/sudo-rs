use sudo_test::{Command, Env, User};
use crate::{Result, SUDOERS_ROOT_ALL, USERNAME, SUDOERS_USER_ALL_ALL, SUDOERS_ALWAYS_LECTURE, SUDOERS_NO_LECTURE, PASSWORD};

#[test]
fn default_lecture_shown_once() -> Result<()> {
    let expected_lecture = format!(
        "\nWe trust you have received the usual lecture from the local System\nAdministrator. It usually boils down to these three things:\n\n    #1) Respect the privacy of others.\n    #2) Think before you type.\n    #3) With great power comes great responsibility."
    );
    let expected_echo = format!(
        "Yeah!"
    );

    let env = Env(["SUDOERS_ROOT_ALL", SUDOERS_USER_ALL_ALL])
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
    .args(["-S", "true"])
    .as_user(USERNAME)
    .stdin(PASSWORD)
    .exec(&env)?;
    assert_eq!(true, output.status().success());

    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            expected_lecture
        );
    }

    let second_sudo = Command::new("sudo")
    .as_user(USERNAME)
    .stdin(PASSWORD)
    .args(["-S", "echo", "Yeah!"])
    .exec(&env)?;

    assert_eq!(true, second_sudo.status().success());
    assert_eq!(Some(0), second_sudo.status().code());
    assert_eq!(second_sudo.stdout().unwrap(), expected_echo);
    Ok(())
}

#[test]
fn lecture_always_shown() -> Result<()> {
    let expected_lecture = format!(
        "\nWe trust you have received the usual lecture from the local System\nAdministrator. It usually boils down to these three things:\n\n    #1) Respect the privacy of others.\n    #2) Think before you type.\n    #3) With great power comes great responsibility."
    );
    // When implemented, switch lecture and check for the new one
    // (so test can fail for sudo-rs).
    let env = Env([
        SUDOERS_ROOT_ALL,
        SUDOERS_ALWAYS_LECTURE,
        "timestamp_timeout=0"])
        .user(USERNAME).build()?;

    let output = Command::new("sudo")
    .as_user(USERNAME)
    .stdin(PASSWORD)
    .args(["-S", "true"])
    .exec(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert_eq!(false, output.status().success());

    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            expected_lecture
        );
    }

    let second_sudo = Command::new("sudo")
    .as_user(USERNAME)
    .stdin(PASSWORD)
    .args(["-S", "ls"])
    .exec(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert_eq!(false, output.status().success());

    if sudo_test::is_original_sudo() {
        assert_contains!(
            second_sudo.stderr(),
            expected_lecture
        );
    }
    Ok(())
}

#[test]
fn lecture_never_shown() -> Result<()> {
    let expected_echo = format!(
        "Yeah!"
    );

    let env = Env([SUDOERS_ROOT_ALL, SUDOERS_USER_ALL_ALL, SUDOERS_NO_LECTURE])
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sudo")
    .args(["-S", "echo", "Yeah!"])
    .as_user(USERNAME)
    .stdin(PASSWORD)
    .exec(&env)?;

    assert_eq!(true, output.status().success());
    assert_eq!(Some(0), output.status().code());
    assert_eq!(output.stdout().unwrap(), expected_echo);
    Ok(())
}
