use sudo_test::{Command, Env};

use crate::USERNAME;

#[test]
fn other_user_does_not_exist() {
    let env = Env("").build();

    let output = Command::new("sudo")
        .args(["-l", "-U", USERNAME])
        .output(&env);

    eprintln!("{}", output.stderr());

    output.assert_exit_code(1);
    let diagnostic = if sudo_test::is_original_sudo() {
        format!("sudo: unknown user {USERNAME}")
    } else {
        format!("sudo-rs: user '{USERNAME}' not found")
    };
    assert_contains!(output.stderr(), diagnostic);
}
