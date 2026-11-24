use sudo_test::{Command, Env, User};

use crate::{OTHER_USERNAME, PASSWORD, USERNAME};

#[test]
fn caching_associated_with_a_tty() {
    let env = Env("ALL ALL=(ALL:ALL) ALL
Defaults timestamp_type=tty")
    .user(User(USERNAME).password(PASSWORD))
    .build();

    // input valid credentials and observe they are cached even though the parent process is different;
    // the first test is very similar to the 'timestamp_timeout' test, except that a tty is connected; hence
    // we do not need to test its successful operation without a tty in this suite of tests
    for test in [
        "sudo -S true; sudo -n true && true",
        "sudo -S true; sh -c 'sudo -n true' && true",
        "sh -c 'sudo -S true'; sudo -n true && true",
    ] {
        Command::new("sh")
            .arg("-c")
            .arg(format!("echo {PASSWORD} | {test}"))
            .as_user(USERNAME)
            .tty(true)
            .output(&env)
            .assert_success();
    }

    // observe that without a tty, we fallback to PPID mode and the credential is not re-used
    for test in [
        "sudo -S true; sh -c 'sudo -n true' && true",
        "sh -c 'sudo -S true'; sudo -n true && true",
        "sh -c 'sudo -S true'; sh -c 'sudo -n true' && true",
    ] {
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("echo {PASSWORD} | {test}"))
            .as_user(USERNAME)
            .tty(false)
            .output(&env);

        if sudo_test::is_original_sudo() {
            assert_contains!(output.stderr(), "a password is required");
        } else {
            assert_contains!(output.stderr(), "interactive authentication is required");
        }
        output.assert_exit_code(1);
    }
}

#[test]
fn caching_associated_with_a_pid() {
    let env = Env("ALL ALL=(ALL:ALL) ALL
Defaults timestamp_type=ppid")
    .user(User(USERNAME).password(PASSWORD))
    .build();

    // this test has some overlap with the one in timestamp_timeout, but this clearly demonstrates that
    // mode=ppid ignores the presence of a TTY
    for test in ["sudo -S true; sudo -n true && true"] {
        for has_tty in [true, false] {
            Command::new("sh")
                .arg("-c")
                .arg(format!("echo {PASSWORD} | {test}"))
                .as_user(USERNAME)
                .tty(has_tty)
                .output(&env)
                .assert_success();
        }
    }

    // whether or not a tty now doesn't matter since ppid mode is always used
    for has_tty in [true, false] {
        for test in [
            "sudo -S true; sh -c 'sudo -n true' && true",
            "sh -c 'sudo -S true'; sudo -n true && true",
            "sh -c 'sudo -S true'; sh -c 'sudo -n true' && true",
        ] {
            let output = Command::new("sh")
                .arg("-c")
                .arg(format!("echo {PASSWORD} | {test}"))
                .as_user(USERNAME)
                .tty(has_tty)
                .output(&env);

            // for some reason this appears on stdout with both ogsudo and sudo-rs
            let output_msg = if !has_tty {
                output.stderr()
            } else {
                output.stdout_unchecked()
            };

            if sudo_test::is_original_sudo() {
                assert_contains!(output_msg, "a password is required");
            } else {
                assert_contains!(output_msg, "interactive authentication is required");
            }
            output.assert_exit_code(1);
        }
    }
}

#[test]
fn global_credentials_expire() {
    let env = Env("ALL ALL=(ALL:ALL) ALL
Defaults timestamp_type=global
Defaults timestamp_timeout=0.1")
    .user(User(USERNAME).password(PASSWORD))
    .build();

    Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    // create a separate process with no tty
    Command::new("sh")
        .arg("-c")
        .arg("sudo -n true")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    // the timeout is 6 seconds, measured from the last successful use above
    let output = Command::new("sh")
        .arg("-c")
        .arg("sleep 10; sudo -n true")
        .as_user(USERNAME)
        .output(&env);

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "a password is required");
    } else {
        assert_contains!(output.stderr(), "interactive authentication is required");
    }
    output.assert_exit_code(1);
}

#[test]
fn global_ignores_process_and_tty() {
    // a global record is tied only to the user, so neither the process nor the tty should
    // affect whether it is reused. Try every combination of having a tty when authenticating
    // and when using the credentials. Put the sudo calls in different processes.
    for (auth_tty, use_tty) in [(true, true), (true, false), (false, true), (false, false)] {
        for test in [
            "sudo -S true; sh -c 'sudo -n true' && true",
            "sh -c 'sudo -S true'; sudo -n true && true",
            "sh -c 'sudo -S true'; sh -c 'sudo -n true' && true",
        ] {
            // use a fresh env for each case, since a global record would carry over
            let env = Env("ALL ALL=(ALL:ALL) ALL
Defaults timestamp_type=global")
            .user(User(USERNAME).password(PASSWORD))
            .build();

            Command::new("sh")
                .arg("-c")
                .arg(format!("echo {PASSWORD} | {test}"))
                .as_user(USERNAME)
                .tty(auth_tty)
                .output(&env)
                .assert_success();

            let output = Command::new("sh")
                .arg("-c")
                .arg("sudo -n true")
                .as_user(USERNAME)
                .tty(use_tty)
                .output(&env);

            output.assert_success();

            // sudo's messages appear on stdout with a tty and on stderr without one, so
            // check both streams to show that the record is reused silently
            assert_eq!(output.stdout_unchecked(), "");
            assert_eq!(output.stderr(), "");
        }
    }
}

#[test]
fn global_per_user() {
    let env = Env("ALL ALL=(ALL:ALL) ALL
Defaults timestamp_type=global")
    .user(User(USERNAME).password(PASSWORD))
    .user(User(OTHER_USERNAME).password(PASSWORD))
    .build();

    // set up a global credential
    Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    // check that sudo works without a password
    let output = Command::new("sh")
        .arg("-c")
        .arg("sudo -n true")
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();

    // but not for a different user
    let output = Command::new("sh")
        .arg("-c")
        .arg("sudo -n true")
        .as_user(OTHER_USERNAME)
        .output(&env);

    output.assert_exit_code(1);
    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "a password is required");
    } else {
        assert_contains!(output.stderr(), "interactive authentication is required");
    }
}

#[test]
fn global_wrong_password() {
    let env = Env("ALL ALL=(ALL:ALL) ALL
Defaults timestamp_type=global")
    .user(User(USERNAME).password(PASSWORD))
    .build();

    // exhaust the number of password attempts
    let output = Command::new("sh")
        .arg("-c")
        .arg("for i in 1 2 3; do echo wrong-password; done | sudo -S true")
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);
    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "3 incorrect password attempts");
    } else {
        assert_contains!(
            output.stderr(),
            "maximum 3 incorrect authentication attempts"
        );
    }

    // sudo should not work after password failure
    let output = Command::new("sh")
        .arg("-c")
        .arg("sudo -n true")
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);
    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "a password is required");
    } else {
        assert_contains!(output.stderr(), "interactive authentication is required");
    }
}

#[test]
fn non_overlapping_jurisdictions() {
    for [mode1, mode2] in [
        ["tty", "ppid"],
        ["ppid", "tty"],
        ["tty", "global"],
        ["global", "tty"],
        ["ppid", "global"],
        ["global", "ppid"],
    ] {
        let env = Env(format!(
            "ALL ALL=(ALL:ALL) ALL
Defaults!/usr/bin/false timestamp_type={mode1}
Defaults!/usr/bin/true timestamp_type={mode2}"
        ))
        .user(User(USERNAME).password(PASSWORD))
        .build();

        let output = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "echo {PASSWORD} | sudo -S false; sudo -n true && true"
            ))
            .as_user(USERNAME)
            .tty(true)
            .output(&env);

        // for some reason this appears on stdout
        if sudo_test::is_original_sudo() {
            assert_contains!(output.stdout_unchecked(), "a password is required");
        } else {
            assert_contains!(
                output.stdout_unchecked(),
                "interactive authentication is required"
            );
        }
        output.assert_exit_code(1);
    }
}
