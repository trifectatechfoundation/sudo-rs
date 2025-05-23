use sudo_test::{Command, Env, User};

use crate::{HOSTNAME, OTHER_USERNAME, PASSWORD, USERNAME};

#[test]
fn invoking_user_has_list_perms() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) list"))
        .user(User(USERNAME).password(PASSWORD))
        .hostname(HOSTNAME)
        .build();

    let output = Command::new("sudo")
        .args(["-S", "-l"])
        .stdin(PASSWORD)
        .as_user(USERNAME)
        .output(&env);

    assert_contains!(
        output.stdout(),
        format!("User {USERNAME} may run the following commands on {HOSTNAME}:")
    );
}

#[test]
fn invoking_user_has_list_perms_nopasswd() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) NOPASSWD: list"))
        .user(USERNAME)
        .hostname(HOSTNAME)
        .build();

    let output = Command::new("sudo")
        .arg("-l")
        .as_user(USERNAME)
        .output(&env);

    assert_contains!(
        output.stdout(),
        format!(
            "User {USERNAME} may run the following commands on {HOSTNAME}:
    (ALL : ALL) NOPASSWD: list"
        )
    );
}

#[test]
fn other_user_has_list_perms_but_invoking_user_has_not() {
    let env = Env(format!("{OTHER_USERNAME} ALL=(ALL:ALL) list"))
        .user(User(USERNAME).password(PASSWORD))
        .user(OTHER_USERNAME)
        .hostname(HOSTNAME)
        .build();

    let output = Command::new("sudo")
        .args(["-S", "-l", "-U", OTHER_USERNAME])
        .stdin(PASSWORD)
        .as_user(USERNAME)
        .output(&env);

    assert!(!output.status().success());
    assert_contains!(
        output.stderr(),
        format!(
            "Sorry, user {USERNAME} is not allowed to execute 'list' as {OTHER_USERNAME} on {HOSTNAME}."
        )
    );
}

#[test]
fn invoking_user_has_list_perms_but_other_user_does_not_have_sudo_perms() {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) NOPASSWD: list"))
        .user(User(USERNAME).password(PASSWORD))
        .user(OTHER_USERNAME)
        .hostname(HOSTNAME)
        .build();

    let output = Command::new("sudo")
        .args(["-S", "-l", "-U", OTHER_USERNAME])
        .stdin(PASSWORD)
        .as_user(USERNAME)
        .output(&env);

    assert_contains!(
        output.stdout(),
        format!("User {OTHER_USERNAME} is not allowed to run sudo on {HOSTNAME}.")
    );
}
