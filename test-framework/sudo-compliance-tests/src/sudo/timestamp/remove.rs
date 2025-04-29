use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

#[test]
fn is_limited_to_a_single_user() {
    let second_user = "ghost";
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(PASSWORD))
        .user(User(second_user).password(PASSWORD))
        .build();

    let child = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; touch /tmp/barrier1; until [ -f /tmp/barrier2 ]; do sleep 1; done; sudo -S true && true"
        ))
        .as_user(USERNAME)
        .spawn(&env);

    Command::new("sh")
        .arg("-c")
        .arg("until [ -f /tmp/barrier1 ]; do sleep 1; done; sudo -K && touch /tmp/barrier2")
        .as_user(second_user)
        .output(&env)
        .assert_success();

    child.wait().assert_success();
}

#[test]
fn has_a_user_global_effect() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let child = Command::new("sh")
    .arg("-c")
    .arg(format!(
        "echo {PASSWORD} | sudo -S true; touch /tmp/barrier1; until [ -f /tmp/barrier2 ]; do sleep 1; done; echo | sudo -S true && true"
    ))
    .as_user(USERNAME)
    .spawn(&env);

    Command::new("sh")
        .arg("-c")
        .arg("until [ -f /tmp/barrier1 ]; do sleep 1; done; sudo -K && touch /tmp/barrier2")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    let output = child.wait();

    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "1 incorrect password attempt"
    } else {
        "incorrect authentication attempt"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn also_works_locally() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    // input valid credentials
    // invalidate them
    // try to sudo without a password
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -K; sudo true && true"
        ))
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "a password is required"
    } else {
        "Authentication failed"
    };
    assert_contains!(output.stderr(), diagnostic);
}
