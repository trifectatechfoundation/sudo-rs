use sudo_test::{Command, Env, TextFile, User};

use crate::{Result, PASSWORD, USERNAME};

mod user_list;

#[test]
fn cannot_sudo_if_sudoers_file_is_world_writable() -> Result<()> {
    let env = Env(TextFile("").chmod("446")).build()?;

    let output = Command::new("sudo").arg("true").exec(&env)?;
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "/etc/sudoers is world writable");
    }

    Ok(())
}

#[ignore]
#[test]
fn user_specifications_evaluated_bottom_to_top() -> Result<()> {
    let env = Env(format!(
        r#"{USERNAME} ALL=(ALL:ALL) NOPASSWD: ALL
{USERNAME} ALL=(ALL:ALL) ALL"#
    ))
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .exec(&env)?;
    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "no password was provided");
    }

    Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .exec(&env)?
        .assert_success()
}
