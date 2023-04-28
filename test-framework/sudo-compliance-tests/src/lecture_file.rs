use sudo_test::{Command, Env};
use crate::{Result, SUDOERS_ROOT_ALL, USERNAME, SUDOERS_NEW_LECTURE, SUDOERS_NEW_LECTURE_USER};

#[test]
fn default_lecture_message() -> Result<()> {
    let expected = format!(
        "\nWe trust you have received the usual lecture from the local System\nAdministrator. It usually boils down to these three things:\n\n    #1) Respect the privacy of others.\n    #2) Think before you type.\n    #3) With great power comes great responsibility."
    );
    let env = Env(SUDOERS_ROOT_ALL).user(USERNAME).build()?;

    let output = Command::new("sudo")
    .arg(format!("ls"))
    .as_user(USERNAME)
    .exec(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert_eq!(false, output.status().success());
    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            expected
        );
    }
    Ok(())
}

#[test]
#[ignore]
fn new_lecture_message() -> Result<()> {
    let new_lecture = format!("I <3 sudo");
    let env = Env([
        SUDOERS_ROOT_ALL,
        SUDOERS_NEW_LECTURE])
        .file("/etc/sudo_lecture", new_lecture)
        .user(USERNAME).build()?;

    let output = Command::new("sudo")
    .as_user(USERNAME)
    .arg(format!("ls"))
    .exec(&env)?;
    assert_eq!(Some(1), output.status().code());
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
        .user(USERNAME).build()?;

    let output = Command::new("sudo")
    .as_user(USERNAME)
    .arg(format!("ls"))
    .exec(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert_eq!(false, output.status().success());
    assert_contains!(
        output.stderr(),
        "I <3 sudo"
    );
    Ok(())
}

#[test]
fn default_lecture_for_unspecified_user() -> Result<()> {
    let expected = format!(
        "\nWe trust you have received the usual lecture from the local System\nAdministrator. It usually boils down to these three things:\n\n    #1) Respect the privacy of others.\n    #2) Think before you type.\n    #3) With great power comes great responsibility."
    );
    let new_lecture = format!("I <3 sudo");
    let env = Env([
        SUDOERS_ROOT_ALL,
        SUDOERS_NEW_LECTURE_USER])
        .file("/etc/sudo_lecture", new_lecture)
        .user(USERNAME).user("other_user").build()?;

    let output = Command::new("sudo")
    .as_user("other_user")
    .arg(format!("ls"))
    .exec(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert_eq!(false, output.status().success());
    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            expected
        );
    }
    Ok(())
}
