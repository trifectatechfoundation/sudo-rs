use sudo_test::{BIN_TRUE, Command, Env};

use crate::{SUDOERS_ALL_ALL_NOPASSWD, SUDOERS_USER_ALL_ALL, USERNAME, helpers::Rsyslogd};

#[test]
fn sudo_logs_every_executed_command() {
    for log_allowed in ["", "Defaults log_allowed"] {
        let env = Env([log_allowed, SUDOERS_ALL_ALL_NOPASSWD]).build();
        let rsyslog = Rsyslogd::start(&env);

        let auth_log = rsyslog.auth_log();
        assert_eq!("", auth_log);

        Command::new("sudo")
            .arg("true")
            .output(&env)
            .assert_success();

        let auth_log = rsyslog.auth_log();
        assert_contains!(auth_log, format!("COMMAND={BIN_TRUE}"));
    }
}

#[test]
fn sudo_respects_log_allowed() {
    let env = Env(["Defaults !log_allowed", SUDOERS_ALL_ALL_NOPASSWD]).build();
    let rsyslog = Rsyslogd::start(&env);

    let auth_log = rsyslog.auth_log();
    assert_eq!("", auth_log);

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();

    let auth_log = rsyslog.auth_log();
    assert_not_contains!(auth_log, format!("COMMAND="));
}

#[test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "Logging not really functional on FreeBSD even with og-sudo"
)]
fn sudo_logs_every_failed_authentication_attempt() {
    let env = Env(SUDOERS_USER_ALL_ALL).user(USERNAME).build();
    let rsyslog = Rsyslogd::start(&env);

    let auth_log = rsyslog.auth_log();
    assert_eq!("", auth_log);

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .output(&env);

    assert!(!output.status().success());

    let auth_log = rsyslog.auth_log();
    assert_contains!(auth_log, "auth could not identify password");
}
