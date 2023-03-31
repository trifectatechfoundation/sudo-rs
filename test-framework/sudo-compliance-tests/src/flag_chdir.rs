use sudo_test::{Command, Env, TextFile, User};
use crate::{Result, PASSWORD, SUDOERS_ROOT_ALL_NOPASSWD, USERNAME, SUDOERS_ALL_ALL_NOPASSWD};

#[test]
#[ignore]
fn cwd_not_set_cannot_change_dir() -> Result<()> {
    let env = Env(TextFile(SUDOERS_ALL_ALL_NOPASSWD)).build()?;

    let output = Command::new("sudo").args(["--chdir", "/root", "pwd"]).exec(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert_eq!(false, output.status().success());
    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "you are not permitted to use the -D option with /bin/pwd");
    }

    Ok(())
}

#[test]
#[ignore]
fn cwd_set_to_glob_change_dir() -> Result<()> {
    let env = Env(TextFile("ALL ALL=(ALL:ALL) CWD=* NOPASSWD: ALL")).build()?;
    let output = Command::new("sh").args(["-c", "cd /; sudo --chdir /root pwd"]).exec(&env)?;
    assert_eq!(Some(0), output.status().code());
    assert_eq!(true, output.status().success());
    assert_contains!(output.stdout()?, "/root");

    Ok(())
}
