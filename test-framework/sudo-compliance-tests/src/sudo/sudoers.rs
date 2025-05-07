use std::collections::HashSet;

use sudo_test::{Command, Env, TextFile, User, ETC_DIR, ROOT_GROUP};

use crate::{PASSWORD, SUDOERS_ROOT_ALL_NOPASSWD, USERNAME};

mod cmnd;
mod cmnd_alias;
mod cwd;
mod env;
mod host_alias;
mod host_list;
mod include;
mod includedir;
mod run_as;
mod runas_alias;
mod secure_path;
mod specific_defaults;
mod timestamp_timeout;
mod user_list;

const KEYWORDS: &[&str] = &[
    "ALL",
    "CHROOT",
    "CWD",
    "Cmnd_Alias",
    "Defaults",
    "FOLLOW",
    "Host_Alias",
    "INTERCEPT",
    "LOG_INPUT",
    "LOG_OUTPUT",
    "MAIL",
    "NOEXEC",
    "NOFOLLOW",
    "NOINTERCEPT",
    "NOLOG_INPUT",
    "NOLOG_OUTPUT",
    "NOMAIL",
    "NOPASSWD",
    "NOSETENV",
    "NOTAFTER",
    "NOTBEFORE",
    "PASSWD",
    "Runas_Alias",
    "SETENV",
    "TIMEOUT",
    "User_Alias",
    "env_check",
    "env_delete",
    "env_editor",
    "env_keep",
    "include",
    "includedir",
    "secure_path",
    "timestamp_timeout",
    "use_pty",
];

const KEYWORDS_ALIAS_BAD: &[&str] = &[
    "ALL",
    "CHROOT",
    "CWD",
    "Cmnd_Alias",
    "Defaults",
    "Host_Alias",
    "NOTAFTER",
    "NOTBEFORE",
    "Runas_Alias",
    "TIMEOUT",
    "User_Alias",
    "env_check",
    "env_delete",
    "env_editor",
    "env_keep",
    "include",
    "includedir",
    "secure_path",
    "timestamp_timeout",
    "use_pty",
];

fn keywords_alias_good() -> HashSet<&'static str> {
    KEYWORDS
        .iter()
        .filter(|keyword| !KEYWORDS_ALIAS_BAD.contains(keyword))
        .copied()
        .collect()
}

#[test]
fn cannot_sudo_if_sudoers_file_is_world_writable() {
    let env = Env(TextFile(SUDOERS_ROOT_ALL_NOPASSWD).chmod("446")).build();

    let output = Command::new("sudo").arg("true").output(&env);
    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("{ETC_DIR}/sudoers is world writable")
    } else {
        format!("invalid configuration: {ETC_DIR}/sudoers cannot be world-writable")
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn cannot_sudo_if_sudoers_file_is_group_writable() {
    let env = Env(TextFile(SUDOERS_ROOT_ALL_NOPASSWD)
        .chmod("464")
        .chown("root:1234"))
    .user(User(USERNAME).password(PASSWORD))
    .build();

    let output = Command::new("sudo").arg("true").output(&env);
    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("{ETC_DIR}/sudoers is owned by gid 1234, should be 0")
    } else {
        format!("invalid configuration: {ETC_DIR}/sudoers cannot be group-writable")
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn can_sudo_if_sudoers_file_is_owner_writable() {
    let env = Env(TextFile(SUDOERS_ROOT_ALL_NOPASSWD).chmod("644")).build();

    let output = Command::new("sudo").arg("true").output(&env);
    output.assert_success();
}

#[test]
fn cannot_sudo_if_sudoers_file_is_not_owned_by_root() {
    let env = Env(TextFile(SUDOERS_ROOT_ALL_NOPASSWD).chown(format!("1234:{ROOT_GROUP}")))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let output = Command::new("sudo").arg("true").output(&env);
    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("{ETC_DIR}/sudoers is owned by uid 1234, should be 0")
    } else {
        format!("invalid configuration: {ETC_DIR}/sudoers must be owned by root")
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn user_specifications_evaluated_bottom_to_top() {
    let env = Env(format!(
        r#"{USERNAME} ALL=(ALL:ALL) NOPASSWD: ALL
{USERNAME} ALL=(ALL:ALL) ALL"#
    ))
    .user(User(USERNAME).password(PASSWORD))
    .build();

    let output = Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .output(&env);
    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "no password was provided"
    } else {
        "incorrect authentication attempt"
    };
    assert_contains!(output.stderr(), diagnostic);

    Command::new("sudo")
        .args(["-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)
        .assert_success();
}

#[test]
fn accepts_sudoers_file_that_has_no_trailing_newline() {
    let env = Env(TextFile(SUDOERS_ROOT_ALL_NOPASSWD).no_trailing_newline())
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn negated_defaults_errors() {
    let env = Env("Defaults !unsupported\nDefaults !passwd_tries").build();

    let output = Command::new("sudo").arg("true").output(&env);
    output.assert_exit_code(1);

    let diagnostic1 = if sudo_test::is_original_sudo() {
        "unknown defaults entry \"unsupported\""
    } else {
        "unknown setting: 'unsupported'"
    };
    assert_contains!(output.stderr(), diagnostic1);

    let diagnostic2 = if sudo_test::is_original_sudo() {
        "no value specified for \"passwd_tries\""
    } else {
        "'passwd_tries' cannot be used in a boolean context"
    };
    assert_contains!(output.stderr(), diagnostic2);
}
