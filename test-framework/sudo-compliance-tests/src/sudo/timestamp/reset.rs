use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

#[test]
fn it_works() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    // input valid credentials
    // invalidate them
    // try to sudo without a password
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -k; sudo true && true"
        ))
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

#[test]
fn has_a_local_effect() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let child = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; echo {PASSWORD} | sudo -S true; touch /tmp/barrier1; until [ -f /tmp/barrier2 ]; do sleep 1; done; sudo true && true"
        ))
        .as_user(USERNAME)
        .spawn(&env);

    Command::new("sh")
        .arg("-c")
        .arg("until [ -f /tmp/barrier1 ]; do sleep 1; done; sudo -k; touch /tmp/barrier2")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    child.wait().assert_success();
}

#[test]
fn with_command_prompts_for_password() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -k true && true"
        ))
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

#[test]
fn with_command_failure_does_not_invalidate_credential_cache() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    // the first command, `sudo -S true`, succeeds and caches credentials
    //
    // the second command, `sudo -k true`, prompts for a password and fails because no password is
    // provided
    //
    // the last command, `sudo true`, is expected to work because `sudo -k true` did *not*
    // invalidate the credentials
    //
    // the `[ $? -eq 1 ] || exit 2` bit is there to turn the success of `sudo -k true` (which is
    // expected to fail) into a failure of the entire shell script
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -k true; [ $? -eq 1 ] || exit 2; sudo true && true"
        ))
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn with_command_success_does_not_invalidate_credential_cache() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    // the first command, `sudo -S true`, succeeds and caches credentials
    //
    // the second command, `sudo -k true`, succeeds
    //
    // the last command, `sudo true`, is expected to work because `sudo -k true` did *not*
    // invalidate the credentials
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; echo {PASSWORD} | sudo -k -S true; sudo true && true"
        ))
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn with_command_does_not_cache_credentials() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -k -S true 2>/dev/null || exit 2; sudo true && true"
        ))
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
