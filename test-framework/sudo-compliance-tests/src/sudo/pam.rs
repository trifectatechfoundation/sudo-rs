//! PAM integration tests

use std::collections::HashMap;

use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

#[cfg(target_os = "linux")]
mod env;

const TEST_ENV_EXPECTED_TTY: &str = "SUDO_RS_TEST_ENV_EXPECTED_TTY";
const PAM_ENV_VALUE: &str = "/tmp/PAM_ENV_VALUE";

fn build_pam_capture_env() -> sudo_test::Env {
    Env("ALL ALL=(ALL:ALL) ALL")
        .user(USERNAME)
        .file(
            "/tmp/env",
            r#"#! /bin/sh
umask a+r
/usr/bin/env > "$1"
"#,
        )
        .file(
            "/etc/pam.d/sudo",
            format!(
                r#"auth optional pam_exec.so /bin/sh /tmp/env {PAM_ENV_VALUE}
auth sufficient pam_permit.so"#
            ),
        )
        .build()
}

fn parse_pam_env(stdout: &str) -> HashMap<String, String> {
    let mut pam_env = HashMap::new();

    for line in stdout.lines() {
        if let Some((key, value)) = line.split_once('=') {
            pam_env.insert(key.to_string(), value.to_string());
        }
    }

    pam_env
}

fn assert_pam_tty_is_valid(actual: &str) {
    assert!(!actual.is_empty(), "PAM_TTY must not be empty");
    assert_ne!(actual, "/dev/null", "PAM_TTY must not be /dev/null");
    assert!(
        actual.starts_with("/dev/"),
        "PAM_TTY must be a /dev path, got: {actual}"
    );
}

fn assert_pam_tty_matches_expected(expected: &str, pam_env: &HashMap<String, String>) {
    let actual = pam_env
        .get("PAM_TTY")
        .map(String::as_str)
        .unwrap_or_default();

    if expected.is_empty() {
        assert!(
            actual.is_empty(),
            "PAM_TTY should be empty when command has no tty"
        );
        return;
    }

    assert_pam_tty_is_valid(actual);
    assert_eq!(expected, actual, "PAM_TTY should match controlling TTY");
}

fn parse_expected_tty_and_pam_env(stdout: &str) -> (String, HashMap<String, String>) {
    let mut env = parse_pam_env(stdout);
    let expected = env
        .remove(TEST_ENV_EXPECTED_TTY)
        .expect("expected tty value not found");
    (expected, env)
}

#[test]
fn given_pam_permit_then_no_password_auth_required() {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(USERNAME)
        .file("/etc/pam.d/sudo", "auth sufficient pam_permit.so")
        .build();

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn given_pam_deny_then_password_auth_always_fails() {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(User(USERNAME).password(PASSWORD))
        .file("/etc/pam.d/sudo", "auth requisite pam_deny.so")
        .build();

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env);

    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "3 incorrect password attempts"
    } else {
        "3 incorrect authentication attempts"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn being_root_has_precedence_over_pam() {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file("/etc/pam.d/sudo", "auth requisite pam_deny.so")
        .build();

    Command::new("sudo")
        .args(["true"])
        .output(&env)
        .assert_success();
}

#[test]
fn nopasswd_in_sudoers_has_precedence_over_pam() {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .file("/etc/pam.d/sudo", "auth requisite pam_deny.so")
        .user(USERNAME)
        .build();

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn sudo_uses_correct_service_file() {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file("/etc/pam.d/sudo", "auth sufficient pam_permit.so")
        .file("/etc/pam.d/sudo-i", "auth requisite pam_deny.so")
        .user(USERNAME)
        .build();

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "FreeBSD doesn't use sudo-i PAM context"
)]
fn sudo_dash_i_uses_correct_service_file() {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file("/etc/pam.d/sudo-i", "auth sufficient pam_permit.so")
        .file("/etc/pam.d/sudo", "auth requisite pam_deny.so")
        .user(USERNAME)
        .build();

    Command::new("sudo")
        .args(["-i", "true"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn pam_tty_is_set_when_stdio_fds_are_not_ttys() {
    let env = build_pam_capture_env();

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
exec </dev/null >/dev/null 2>&1
sudo true
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {PAM_ENV_VALUE} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
fn pam_tty_with_stdout_redirect_uses_stdin_tty() {
    let env = build_pam_capture_env();

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
exec >/dev/null
sudo true
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {PAM_ENV_VALUE} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
fn pam_tty_with_stderr_redirect_uses_stdin_tty() {
    let env = build_pam_capture_env();

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
sudo true 2>/dev/null
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {PAM_ENV_VALUE} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
fn pam_tty_with_stdout_and_stderr_redirect_uses_stdin_tty() {
    let env = build_pam_capture_env();

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
sudo true >/dev/null 2>&1
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {PAM_ENV_VALUE} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
fn pam_tty_is_set_when_stdin_is_closed_and_stdio_fds_are_not_ttys() {
    let env = build_pam_capture_env();

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
exec 0<&- >/dev/null 2>&1
sudo true
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {PAM_ENV_VALUE} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
fn pam_tty_is_set_when_stdin_is_closed_even_if_stdout_stderr_are_ttys() {
    let env = build_pam_capture_env();

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
exec 0<&-
sudo true
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {PAM_ENV_VALUE} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
fn pam_tty_is_set_when_stdin_is_devnull_even_if_stdout_stderr_are_ttys() {
    let env = build_pam_capture_env();

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
exec </dev/null
sudo true
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {PAM_ENV_VALUE} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
fn pam_tty_is_not_pts_when_command_has_no_tty() {
    let env = build_pam_capture_env();

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"expected_tty=$(tty -s || echo '')
sudo true
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty"
cat {PAM_ENV_VALUE}"#
        ))
        .as_user(USERNAME)
        .output(&env)
        .stdout();

    println!("stdout: {stdout}");
    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
fn pam_tty_with_stdin_pipe_uses_controlling_tty() {
    let env = build_pam_capture_env();

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
printf 'through-a-pipe\n' | sudo true >/dev/null 2>&1
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {PAM_ENV_VALUE} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
fn pam_tty_is_set_when_stdout_is_closed_even_if_stdin_stderr_are_ttys() {
    let env = build_pam_capture_env();

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
sudo true >&-
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {PAM_ENV_VALUE} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
fn pam_tty_with_stdin_here_string_uses_controlling_tty() {
    let env = build_pam_capture_env();

    let stdout = Command::new("bash")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
sudo -S -p '' true <<< '{PASSWORD}'
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {PAM_ENV_VALUE} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
fn pam_tty_with_background_stdin_here_string_uses_controlling_tty() {
    let env = build_pam_capture_env();

    let stdout = Command::new("bash")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
set -m
sudo -S -p '' true <<< '{PASSWORD}' >/dev/null 2>&1 &
sudo_pid=$!
wait "$sudo_pid"
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {PAM_ENV_VALUE} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}
