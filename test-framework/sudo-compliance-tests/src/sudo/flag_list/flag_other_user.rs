use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

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
        format!("sudo: user '{USERNAME}' not found")
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn other_user_is_self() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) /bin/ls"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let output = Command::new("sudo")
        .args(["-S", "-l", "-U", USERNAME])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env);

    output.assert_success();
}

#[test]
fn current_user_is_root() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) /bin/ls"))
        .user(USERNAME)
        .build();

    let output = Command::new("sudo")
        .args(["-l", "-U", USERNAME])
        .output(&env);

    output.assert_success();
}
