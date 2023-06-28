use sudo_test::{Command, Env};

use crate::{helpers::Rsyslogd, Result, SUDOERS_ALL_ALL_NOPASSWD, SUDOERS_USER_ALL_ALL, USERNAME};

#[test]
#[ignore = "gh421"]
fn sudo_logs_every_executed_command() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;
    let rsyslog = Rsyslogd::start(&env)?;

    let auth_log = rsyslog.auth_log()?;
    assert_eq!("", auth_log);

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()?;

    let auth_log = rsyslog.auth_log()?;
    assert_contains!(auth_log, "COMMAND=/usr/bin/true");

    Ok(())
}

#[test]
fn sudo_logs_every_failed_authentication_attempt() -> Result<()> {
    let env = Env(SUDOERS_USER_ALL_ALL).user(USERNAME).build()?;
    let rsyslog = Rsyslogd::start(&env)?;

    let auth_log = rsyslog.auth_log()?;
    assert_eq!("", auth_log);

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());

    let auth_log = rsyslog.auth_log()?;
    let diagnostic = if sudo_test::is_original_sudo() {
        "auth could not identify password"
    } else {
        "authentication failure"
    };
    assert_contains!(auth_log, diagnostic);

    Ok(())
}
