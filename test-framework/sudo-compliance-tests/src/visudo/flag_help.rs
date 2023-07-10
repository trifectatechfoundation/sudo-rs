use sudo_test::{Command, Env};

use crate::{Result, USERNAME};

#[test]
fn prints_to_stdout() -> Result<()> {
    let env = Env("").user(USERNAME).build()?;

    let long = Command::new("visudo")
        .arg("--help")
        .as_user(USERNAME)
        .output(&env)?
        .stdout()?;

    let short = Command::new("visudo")
        .arg("-h")
        .as_user(USERNAME)
        .output(&env)?
        .stdout()?;

    assert_eq!(short, long);
    assert_contains!(short, "visudo - safely edit the sudoers file");

    Ok(())
}
