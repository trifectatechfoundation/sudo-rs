//! PAM integration tests

use std::collections::HashMap;

use sudo_test::{Command, Directory, Env, User};

use crate::{PASSWORD, USERNAME};

mod env;

const TEST_ENV_EXPECTED_TTY: &str = "SUDO_RS_TEST_ENV_EXPECTED_TTY";

fn test_temp_paths(test_name: &str) -> (String, String, String) {
    let base = std::env::temp_dir().join(format!("sudo-rs-pam-{test_name}-{}", std::process::id()));
    let value = base.join("pam_env_value");
    let log = base.join("repro.log");

    (
        base.to_string_lossy().into_owned(),
        value.to_string_lossy().into_owned(),
        log.to_string_lossy().into_owned(),
    )
}

fn build_pam_capture_env(tmp_dir: &str, pam_env_value: &str) -> sudo_test::Env {
    Env("ALL ALL=(ALL:ALL) ALL")
        .user(USERNAME)
        .directory(Directory(tmp_dir).chmod("777"))
        .file(
            "/etc/pam.d/sudo",
            format!(
                r#"auth optional pam_exec.so log={pam_env_value} /usr/bin/env
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
#[cfg_attr(target_os = "freebsd", ignore = "pam_exec wiring differs on FreeBSD")]
fn pam_tty_with_stdout_redirect_uses_stdin_tty() {
    let (tmp_dir, pam_env_value, repro_log) = test_temp_paths("stdout-not-a-tty");

    let env = build_pam_capture_env(tmp_dir.as_str(), pam_env_value.as_str());

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
exec >{repro_log}
sudo true
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {pam_env_value} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
#[cfg_attr(target_os = "freebsd", ignore = "pam_exec wiring differs on FreeBSD")]
fn pam_tty_with_stderr_redirect_uses_stdin_tty() {
    let (tmp_dir, pam_env_value, repro_log) = test_temp_paths("stderr-not-a-tty");

    let env = build_pam_capture_env(tmp_dir.as_str(), pam_env_value.as_str());

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
sudo true 2>{repro_log}
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {pam_env_value} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
#[cfg_attr(target_os = "freebsd", ignore = "pam_exec wiring differs on FreeBSD")]
fn pam_tty_with_stdout_and_stderr_redirect_uses_stdin_tty() {
    let (tmp_dir, pam_env_value, repro_log) = test_temp_paths("stdout-and-stderr-not-ttys");

    let env = build_pam_capture_env(tmp_dir.as_str(), pam_env_value.as_str());

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
sudo true >{repro_log} 2>&1
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {pam_env_value} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
#[cfg_attr(target_os = "freebsd", ignore = "pam_exec wiring differs on FreeBSD")]
fn pam_tty_is_set_when_stdin_is_closed_even_if_stdout_stderr_are_ttys() {
    let (tmp_dir, pam_env_value, _repro_log) = test_temp_paths("stdin-closed-stdout-stderr-ttys");

    let env = build_pam_capture_env(tmp_dir.as_str(), pam_env_value.as_str());

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
exec 0<&-
sudo true
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {pam_env_value} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
#[cfg_attr(target_os = "freebsd", ignore = "pam_exec wiring differs on FreeBSD")]
fn pam_tty_is_set_when_stdin_is_devnull_even_if_stdout_stderr_are_ttys() {
    let (tmp_dir, pam_env_value, _repro_log) = test_temp_paths("stdin-devnull-stdout-stderr-ttys");

    let env = build_pam_capture_env(tmp_dir.as_str(), pam_env_value.as_str());

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"exec 3>&1
expected_tty=$(tty <&3)
exec </dev/null
sudo true
printf '%s=%s\n' '{TEST_ENV_EXPECTED_TTY}' "$expected_tty" >&3
cat {pam_env_value} >&3"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let (expected, pam_env) = parse_expected_tty_and_pam_env(&stdout);
    assert_pam_tty_matches_expected(&expected, &pam_env);
}

#[test]
#[cfg_attr(target_os = "freebsd", ignore = "pam_exec wiring differs on FreeBSD")]
fn pam_tty_is_not_pts_when_command_has_no_tty() {
    let (tmp_dir, pam_env_value, _repro_log) = test_temp_paths("no-tty");

    let env = build_pam_capture_env(tmp_dir.as_str(), pam_env_value.as_str());

    let pam_env_out = Command::new("sh")
        .arg("-c")
        .arg(format!("sudo true; cat {pam_env_value}"))
        .as_user(USERNAME)
        .output(&env)
        .stdout();

    let pam_env = parse_pam_env(&pam_env_out);
    let pam_tty = pam_env
        .get("PAM_TTY")
        .map(String::as_str)
        .unwrap_or_default();
    assert!(
        !pam_tty.starts_with("/dev/pts/"),
        "PAM_TTY should not resolve to a pts path without an allocated tty: {pam_tty}"
    );
}
