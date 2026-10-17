use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

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
fn non_overlapping_jurisdictions() {
    let modes = ["tty", "ppid"];
    let mut reversed_modes = modes;
    reversed_modes.reverse();

    for [mode1, mode2] in [modes, reversed_modes] {
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
