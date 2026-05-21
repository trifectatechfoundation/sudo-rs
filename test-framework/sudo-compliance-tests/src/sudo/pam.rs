//! PAM integration tests

use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

mod env;

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
#[cfg_attr(target_os = "freebsd", ignore = "pam_echo behavior differs on FreeBSD")]
fn no_tty_pam_text_info_falls_back_to_stdio() {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .user(USERNAME)
        .file(
            "/etc/pam.d/sudo",
            [
                "auth optional pam_echo.so Hello sudo-rs, I am PAM",
                "auth sufficient pam_permit.so",
            ]
            .join("\n"),
        )
        .build();

    Command::new("sh")
        .args(["-c", "sudo true </dev/null >/tmp/repro.log 2>&1"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
#[cfg_attr(target_os = "freebsd", ignore = "pam_echo behavior differs on FreeBSD")]
fn no_tty_pam_text_info_uses_stdio_fallback() {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .file(
            "/etc/pam.d/sudo",
            [
                "auth sufficient pam_permit.so",
                "account sufficient pam_permit.so",
                "session optional pam_echo.so Hello sudo-rs, I am PAM",
                "session sufficient pam_permit.so",
            ]
            .join("\n"),
        )
        .user(USERNAME)
        .build();

    Command::new("sh")
        .args(["-c", "sudo true </dev/null"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}
