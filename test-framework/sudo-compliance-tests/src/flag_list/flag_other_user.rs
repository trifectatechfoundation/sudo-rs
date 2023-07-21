use sudo_test::{Command, Env};

use crate::{Result, USERNAME};

#[test]
fn other_user_does_not_exist() -> Result<()> {
    let env = Env("").build()?;

    let output = Command::new("sudo")
        .args(["-l", "-U", USERNAME])
        .output(&env)?;

    eprintln!("{}", output.stderr());

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostic = if sudo_test::is_original_sudo() {
        format!("sudo: unknown user {USERNAME}")
    } else {
        format!("sudo-rs: user '{USERNAME}' not found")
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}
