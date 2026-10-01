use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

use super::MAX_PASSWORD_SIZE;

#[test]
fn correct_password() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sshpass")
        .args(["-p", PASSWORD, "sudo", "true"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn incorrect_password() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password("strong-password"))
        .build();

    let output = Command::new("sshpass")
        .args(["-p", "incorrect-password", "sudo", "true"])
        .as_user(USERNAME)
        .output(&env);
    assert!(!output.status().success());

    // `sshpass` will override sudo's exit code with the value 5 so we can't check this
    // assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "1 incorrect password attempt");
    }
}

#[test]
fn no_tty() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let output = Command::new("sudo")
        .args(["true"])
        .as_user(USERNAME)
        .output(&env);
    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "a terminal is required to read the password"
    } else {
        "A terminal is required to authenticate"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn prompt_on_same_tty_is_serialized() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    // sshpass only answers one prompt: the second sudo must wait and reuse the timestamp
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "timeout 10 sshpass -p {PASSWORD} sh -c 'sudo touch /tmp/first | sudo touch /tmp/second' || exit $?
test -f /tmp/first && test -f /tmp/second"
        ))
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn prompt_on_another_tty_does_not_block() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    // the first sudo never gets its password (the -P prompt never matches), so it stays
    // at the prompt on its own pty while a second sudo authenticates on a different pty
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "sshpass -P never-matching-prompt -p {PASSWORD} sudo true >/dev/null 2>&1 &
holder=$!
until pgrep -x sudo >/dev/null; do sleep 0.1; done
sleep 1
timeout 10 sshpass -p {PASSWORD} sudo true
ret=$?
kill $holder || exit 99
exit $ret"
        ))
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn longest_possible_password_works() {
    let password = "a".repeat(MAX_PASSWORD_SIZE);

    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(&password))
        .build();

    Command::new("sshpass")
        .args(["-p", &password, "sudo", "true"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
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
        let output = Command::new("sshpass")
            .args(["-p", &input, "sudo", "true"])
            .as_user(USERNAME)
            .output(&env);

        assert!(!output.status().success());
        // `sshpass` will override sudo's exit code with the value 5 so we can't check this
        // assert_eq!(Some(1), output.status().code());

        let stderr = output.stderr();
        let diagnostic = if sudo_test::is_original_sudo() {
            "sudo: 1 incorrect password attempt"
        } else {
            "Incorrect authentication attempt"
        };
        assert_contains!(stderr, diagnostic);
    }
}
