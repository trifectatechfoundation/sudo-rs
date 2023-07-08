use sudo_test::{Command, Env};

use crate::{Result, USERNAME};

#[test]
fn prints_to_stdout() -> Result<()> {
    let env = Env("").user(USERNAME).build()?;

    let long = Command::new("visudo")
        .arg("--version")
        .as_user(USERNAME)
        .output(&env)?
        .stdout()?;

    let short = Command::new("visudo")
        .arg("-V")
        .as_user(USERNAME)
        .output(&env)?
        .stdout()?;

    assert_eq!(short, long);

    assert_contains!(short, "visudo version");
    
    if sudo_test::is_original_sudo() {
        assert_contains!(short, "visudo grammar version 50");
    }

    Ok(())
}
