use sudo_test::{Command, Env, User};
use crate::{Result, SUDOERS_ROOT_ALL, USERNAME, SUDOERS_NEW_LECTURE, SUDOERS_NEW_LECTURE_USER, USER_ALL_ALL, PASSWORD};

#[test]
#[ignore]
fn default_lecture_shown_once() -> Result<()> {
    let expected_error = format!(
        "\nWe trust you have received the usual lecture from the local System\nAdministrator. It usually boils down to these three things:\n\n    #1) Respect the privacy of others.\n    #2) Think before you type.\n    #3) With great power comes great responsibility."
    );
    let expected_ls = format!(
        "bin\nboot\ndev\netc\nhome\nlib\nlib64\nmedia\nmnt\nopt\nproc\nroot\nrun\nsbin\nsrv\nsys\ntmp\nusr\nvar"
    );
    let env = Env(["ALL ALL=(ALL:ALL) CWD=/root /bin/pwd ", "ferris   ALL=(ALL:ALL) ALL"])
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
            expected_error
        );
    }

    let second_sudo = Command::new("sudo")
    .as_user(USERNAME)
    .stdin(PASSWORD)
    .args(["-S", "ls"])
    .exec(&env)?;

    assert_eq!(true, second_sudo.status().success());
    assert_eq!(Some(0), second_sudo.status().code());
    assert_eq!(second_sudo.stdout().unwrap(), expected_ls);
    Ok(())
}
