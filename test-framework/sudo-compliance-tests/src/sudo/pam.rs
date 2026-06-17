//! PAM integration tests

use std::collections::HashMap;

use sudo_test::{Command, Directory, Env, User};

use crate::{PASSWORD, USERNAME};

mod env;

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
fn sudo_prompt_is_not_set_without_dash_p() {
    if sudo_test::is_original_sudo() {
        eprintln!(
            "skipping: requires sudo behavior from https://github.com/sudo-project/sudo/pull/539"
        );
        // Remove this guard once https://github.com/sudo-project/sudo/pull/539 is merged.
        return;
    }

    let (tmp_dir, pam_env_value, repro_log) = test_temp_paths("sudo-prompt-default");
    let env = build_pam_capture_env(tmp_dir.as_str(), pam_env_value.as_str());

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"sudo true >{repro_log} 2>&1
cat {pam_env_value}"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let pam_env = parse_pam_env(&stdout);
    assert!(
        !pam_env.contains_key("SUDO_PROMPT"),
        "SUDO_PROMPT must not be set when -p is not specified"
    );
}

#[test]
#[cfg_attr(target_os = "freebsd", ignore = "pam_exec wiring differs on FreeBSD")]
fn sudo_prompt_is_not_set_when_dash_p_uses_default_value() {
    if sudo_test::is_original_sudo() {
        eprintln!(
            "skipping: requires sudo behavior from https://github.com/sudo-project/sudo/pull/539"
        );
        // Remove this guard once https://github.com/sudo-project/sudo/pull/539 is merged.
        return;
    }

    let (tmp_dir, pam_env_value, repro_log) = test_temp_paths("sudo-prompt-default-value");
    let env = build_pam_capture_env(tmp_dir.as_str(), pam_env_value.as_str());

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"LC_ALL=C LANG=C sudo -p 'authenticate' true >{repro_log} 2>&1
cat {pam_env_value}"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let pam_env = parse_pam_env(&stdout);
    assert!(
        !pam_env.contains_key("SUDO_PROMPT"),
        "SUDO_PROMPT must not be set when -p uses default prompt value"
    );
}

#[test]
#[cfg_attr(target_os = "freebsd", ignore = "pam_exec wiring differs on FreeBSD")]
fn sudo_prompt_is_set_when_dash_p_is_specified() {
    if sudo_test::is_original_sudo() {
        eprintln!(
            "skipping: requires sudo behavior from https://github.com/sudo-project/sudo/pull/539"
        );
        // Remove this guard once https://github.com/sudo-project/sudo/pull/539 is merged.
        return;
    }

    let (tmp_dir, pam_env_value, repro_log) = test_temp_paths("sudo-prompt-custom");
    let env = build_pam_capture_env(tmp_dir.as_str(), pam_env_value.as_str());

    let stdout = Command::new("sh")
        .arg("-c")
        .arg(format!(
            r#"sudo -p 'CUSTOM_PROMPT' true >{repro_log} 2>&1
cat {pam_env_value}"#
        ))
        .as_user(USERNAME)
        .tty(true)
        .output(&env)
        .stdout();

    let pam_env = parse_pam_env(&stdout);
    let actual = pam_env
        .get("SUDO_PROMPT")
        .map(String::as_str)
        .unwrap_or_default();
    assert_eq!("CUSTOM_PROMPT", actual, "SUDO_PROMPT should match -p value");
}
