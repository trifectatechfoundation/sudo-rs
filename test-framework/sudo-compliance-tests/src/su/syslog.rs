use std::time::Duration;

use sudo_test::{Command, Env, User};

use crate::{helpers::Rsyslogd, PASSWORD, USERNAME};

fn wait_until_rsyslogd_starts_up() {
    std::thread::sleep(Duration::from_secs(1));
}

#[test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "Logging not really functional on FreeBSD even with og-sudo"
)]
fn logs_every_session() {
    let invoking_user = USERNAME;
    let invoking_userid = 1000;
    let target_user = "ghost";
    let target_userid = 1001;
    let env = Env("")
        .user(User(invoking_user).id(invoking_userid))
        .user(User(target_user).password(PASSWORD).id(target_userid))
        .build();
    let rsyslogd = Rsyslogd::start(&env);

    wait_until_rsyslogd_starts_up();

    let output = Command::new("su")
        .arg(target_user)
        .as_user(invoking_user)
        .stdin(PASSWORD)
        .output(&env);

    output.assert_success();

    let auth_log = rsyslogd.auth_log();

    eprintln!("\n--- /var/log/auth.log ---\n{auth_log}\n--- /var/log/auth.log ---\n");

    let tty = "none";
    if sudo_test::is_original_sudo() {
        assert_contains!(
            auth_log,
            format!("(to {target_user}) {invoking_user} on {tty}")
        );
    }

    assert_contains!(
        auth_log,
        format!("pam_unix(su:session): session opened for user {target_user}(uid={target_userid}) by (uid={invoking_userid})")
    );

    assert_contains!(
        auth_log,
        format!("pam_unix(su:session): session closed for user {target_user}")
    );
}

#[test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "Logging not really functional on FreeBSD even with og-sudo"
)]
fn logs_every_failed_authentication_attempt() {
    let invoking_user = USERNAME;
    let invoking_userid = 1000;
    let target_user = "ghost";
    let env = Env("")
        .user(User(invoking_user).id(invoking_userid))
        .user(target_user)
        .build();
    let rsyslogd = Rsyslogd::start(&env);

    wait_until_rsyslogd_starts_up();

    let output = Command::new("su")
        .arg(target_user)
        .as_user(invoking_user)
        .output(&env);

    assert!(!output.status().success());

    let auth_log = rsyslogd.auth_log();

    eprintln!("\n--- /var/log/auth.log ---\n{auth_log}\n--- /var/log/auth.log ---\n");

    assert_contains!(
        auth_log,
        format!("su: pam_unix(su:auth): auth could not identify password for [{target_user}]")
    );

    if sudo_test::is_original_sudo() {
        let tty = "none";
        assert_contains!(
            auth_log,
            format!("FAILED SU (to {target_user}) {invoking_user} on {tty}")
        );
    }
}
