use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

use super::MAX_PASSWORD_SIZE;

#[test]
fn correct_password() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)
        .assert_success();
}

#[test]
fn incorrect_password() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password("strong-password"))
        .build();

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin("incorrect-password")
        .output(&env);
    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "incorrect password attempt"
    } else {
        "incorrect authentication attempt"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn no_password() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .output(&env);
    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "no password was provided"
    } else {
        "incorrect authentication attempt"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn longest_possible_password_works() {
    let password = "a".repeat(MAX_PASSWORD_SIZE);

    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(&password))
        .build();

    Command::new("sudo")
        .args(["-S", "true"])
        .stdin(password)
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn input_longer_than_max_pam_response_size_is_handled_gracefully() {
    let env = Env("ALL ALL=(ALL:ALL) ALL").user(USERNAME).build();

    let input = "a".repeat(5 * MAX_PASSWORD_SIZE / 2);
    let output = Command::new("sudo")
        .args(["-S", "true"])
        .stdin(input)
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        if cfg!(target_os = "freebsd") {
            assert_contains!(stderr, "sudo: 1 incorrect password attempt");
        } else {
            assert_contains!(stderr, "sudo: 2 incorrect password attempts");
        }
    } else {
        assert_contains!(stderr, "Incorrect password attempt");
        assert_not_contains!(stderr, "panic");
    }
}

#[test]
fn input_longer_than_password_should_not_be_accepted_as_correct_password() {
    let password = "a".repeat(MAX_PASSWORD_SIZE);
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(password))
        .build();

    let input_sizes = [MAX_PASSWORD_SIZE + 1, MAX_PASSWORD_SIZE + 2];

    for input_size in input_sizes {
        let input = "a".repeat(input_size);
        let output = Command::new("sudo")
            .args(["-S", "true"])
            .stdin(input)
            .as_user(USERNAME)
            .output(&env);

        output.assert_exit_code(1);

        let stderr = output.stderr();
        if sudo_test::is_original_sudo() {
            assert_contains!(stderr, "sudo: 1 incorrect password attempt");
        } else {
            assert_contains!(stderr, "Incorrect password attempt");
        }
    }
}
