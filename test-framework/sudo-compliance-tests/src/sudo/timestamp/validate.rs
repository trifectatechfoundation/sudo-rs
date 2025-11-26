use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

#[test]
fn revalidation() {
    let env = Env(format!(
        "{USERNAME} ALL=(ALL:ALL) ALL
Defaults timestamp_timeout=0.1"
    ))
    .user(User(USERNAME).password(PASSWORD))
    .build();

    // input valid credentials
    // revalidate credentials a few times
    // sudo without a password, using re-validated credentials
    Command::new("sh")
    .arg("-c")
    .arg(format!(
        "set -e; echo {PASSWORD} | sudo -S true; for i in $(seq 1 5); do sleep 3; sudo -v; done; sudo true && true"
    ))
    .as_user(USERNAME)
    .output(&env)
    .assert_success();
}

#[test]
fn prompts_for_password() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let output = Command::new("sudo")
        .arg("-v")
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "a password is required"
    } else {
        "A terminal is required to read the password"
    };
    assert_contains!(output.stderr(), diagnostic);
}
