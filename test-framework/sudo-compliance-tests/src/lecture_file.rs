use sudo_test::{Command, Env};
use crate::{Result, SUDOERS_ROOT_ALL, SUDOERS_NEW_LECTURE};

#[test]
fn default_lecture_message() -> Result<()> {
    let expected = format!(
        "\nWe trust you have received the usual lecture from the local System\nAdministrator. It usually boils down to these three things:\n\n    #1) Respect the privacy of others.\n    #2) Think before you type.\n    #3) With great power comes great responsibility."
    );
    let env = Env(SUDOERS_ROOT_ALL).user("ferris").build()?;

    let output = Command::new("sudo")
    .arg(format!("ls"))
    .as_user("ferris")
    .exec(&env)?;
    assert_eq!(Some(1), output.status().code());
    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            expected
        );
    }
    Ok(())
}

#[test]
fn new_lecture_message() -> Result<()> {
    let new_lecture = format!("I <3 sudo");
    let env = Env([
        SUDOERS_ROOT_ALL,
        SUDOERS_NEW_LECTURE])
        .file("/etc/sudo_lecture", new_lecture)
        .user("ferris").build()?;

    let output = Command::new("sudo")
    .as_user("ferris")
    .arg(format!("ls"))
    .exec(&env)?;
    assert_eq!(Some(1), output.status().code());
    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "I <3 sudo"
        );
    }
    Ok(())
}
