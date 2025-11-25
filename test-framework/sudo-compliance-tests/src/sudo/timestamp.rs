use sudo_test::{Command, Env, User};

use crate::{PASSWORD, SUDO_RS_IS_UNSTABLE, USERNAME};

mod remove;
mod reset;
mod validate;

#[test]
fn credential_caching_works() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; echo {PASSWORD} | sudo -S true; sudo true && true"
        ))
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn by_default_credential_caching_is_local() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sh")
        .arg("-c")
        .arg(format!("set -e; echo {PASSWORD} | sudo -S true && true"))
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    let output = Command::new("sudo")
        .arg("true")
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
fn credential_cache_is_shared_with_child_shell() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; echo {PASSWORD} | sudo -S true; sh -c 'sudo true && true' && true"
        ))
        .as_user(USERNAME)
        // XXX unclear why this and the tests that follow need a pseudo-TTY allocation to pass
        .tty(true)
        .output(&env)
        .assert_success();
}

#[test]
fn credential_cache_is_shared_with_parent_shell() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; sh -c 'echo {PASSWORD} | sudo -S true && true'; sudo true && true"
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .assert_success();
}

#[test]
fn credential_cache_is_shared_between_sibling_shells() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; sh -c 'echo {PASSWORD} | sudo -S true && true'; sh -c 'sudo true' && true"
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .assert_success();
}

#[test]
fn cached_credential_applies_to_all_target_users() {
    let second_target_user = "ghost";
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .user(second_target_user)
        .build();

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -e; echo {PASSWORD} | sudo -S true; sudo -u {second_target_user} true && true"
        ))
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn cached_credential_not_shared_with_target_user_that_are_not_self() {
    let second_target_user = "ghost";
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(PASSWORD))
        .user(second_target_user)
        .build();

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -u {second_target_user} -S true; sudo -u {second_target_user} env '{SUDO_RS_IS_UNSTABLE}' sudo -S true && true"
        ))
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "a password is required"
    } else {
        "Password is required"
    };

    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn cached_credential_shared_with_target_user_that_is_self_on_the_same_tty() {
    let env = Env([
        "Defaults !use_pty".to_string(),
        format!("{USERNAME} ALL=(ALL:ALL) ALL"),
    ])
    .user(User(USERNAME).password(PASSWORD))
    .build();

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -u {USERNAME} env '{SUDO_RS_IS_UNSTABLE}' sudo -n true && true"
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .assert_success();
}

#[test]
fn cached_credential_not_shared_with_self_across_ttys() {
    let env = Env([
        "Defaults use_pty".to_string(),
        format!("{USERNAME} ALL=(ALL:ALL) ALL"),
    ])
    .user(User(USERNAME).password(PASSWORD))
    .build();

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -u {USERNAME} sudo -n true && true"
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env);

    output.assert_exit_code(1);
}

#[test]
fn cached_credential_not_shared_between_auth_users() {
    const PASSWORD: &str = "passw0rd";
    const PASSWORD2: &str = "notr00t";

    let env = Env(format!(
        "Defaults targetpw\nDefaults passwd_tries=1\n{USERNAME} ALL=(ALL:ALL) ALL"
    ))
    .user(User(USERNAME))
    .user(User("user1").password(PASSWORD))
    .user(User("user2").password(PASSWORD2))
    .build();

    // Enter password for first user to cache credential
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S -u user1 true"))
        .as_user(USERNAME)
        .output(&env);
    output.assert_success();

    // Cached credential not used when password for different user necessary
    let output = Command::new("sh")
        .arg("-c")
        .arg("sudo -u user2 true")
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
fn double_negation_also_equals_never() {
    let env = Env([
        "Defaults !!use_pty".to_string(),
        format!("{USERNAME} ALL=(ALL:ALL) ALL"),
    ])
    .user(User(USERNAME).password(PASSWORD))
    .build();

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S true; sudo -u {USERNAME} sudo -n true && true"
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env);

    output.assert_exit_code(1);
}
