use sudo_test::{Child, Command, Env};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, SUDOERS_USER_ALL_ALL, USERNAME};

struct Rsyslogd<'a> {
    _child: Child,
    env: &'a Env,
}

impl<'a> Rsyslogd<'a> {
    fn start(env: &'a Env) -> Result<Self> {
        let child = Command::new("rsyslogd").arg("-n").spawn(env)?;
        Ok(Self { _child: child, env })
    }

    /// returns the contents of `/var/auth.log`
    fn auth_log(&self) -> Result<String> {
        let path = "/var/log/auth.log";
        Command::new("sh")
            .arg("-c")
            .arg(format!("[ ! -f {path} ] || cat {path}"))
            .output(self.env)?
            .stdout()
    }
}

impl Drop for Rsyslogd<'_> {
    fn drop(&mut self) {
        // need to kill the daemon or `Env::drop` won't properly `stop` the docker container
        let _ = Command::new("sh")
            .args(["-c", "kill -9 $(pidof rsyslogd)"])
            .output(self.env);
    }
}

#[test]
fn rsyslogd_works() -> Result<()> {
    let env = Env("").build()?;
    let rsyslog = Rsyslogd::start(&env)?;

    let auth_log = rsyslog.auth_log()?;
    assert_eq!("", auth_log);

    Command::new("useradd")
        .arg("ferris")
        .output(&env)?
        .assert_success()?;

    let auth_log = rsyslog.auth_log()?;
    assert_contains!(auth_log, "useradd");

    Ok(())
}

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
