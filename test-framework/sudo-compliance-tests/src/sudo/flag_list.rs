use sudo_test::{
    Command, Env, TextFile, User, BIN_FALSE, BIN_LS, BIN_PWD, BIN_SUDO, BIN_TRUE, ETC_SUDOERS,
};

use crate::{Result, PANIC_EXIT_CODE, PASSWORD, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

mod credential_caching;
mod flag_other_user;
mod long_format;
mod needs_auth;
mod nopasswd;
mod not_allowed;
mod short_format;
mod sudoers_list;

// sudo-rs doesn't yet support showing Defaults in the `-l` output, so strip
// them with og-sudo to get the same output between both.
fn strip_matching_defaults_message(s: &str) -> &str {
    if s.starts_with("Matching Defaults entries for") {
        s.split_once("\n")
            .unwrap()
            .1
            .split_once("\n")
            .unwrap()
            .1
            .strip_prefix("\n")
            .unwrap()
    } else {
        s
    }
}

#[test]
fn root_cannot_use_list_when_empty_sudoers() {
    let hostname = "container";
    let env = Env("").hostname(hostname).build();

    let output = Command::new("sudo").arg("-l").output(&env);

    let (expected, actual);

    // this is very strange behaviour
    if sudo_test::is_original_sudo() {
        expected = format!("User root is not allowed to run sudo on {hostname}.");
        actual = output.stdout();
    } else {
        expected = format!("Sorry, user root may not run sudo on {hostname}.");
        actual = output.stderr().to_string();
    }
    assert_contains!(actual, expected);
}

#[test]
fn regular_user_can_use_list_regardless_of_which_command_is_allowed_by_sudoers() {
    let hostname = "container";
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) /command/does/not/matter"))
        .user(User(USERNAME).password(PASSWORD))
        .hostname(hostname)
        .build();

    let output = Command::new("sudo")
        .args(["-S", "-l"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env);

    assert_contains!(
        output.stdout(),
        format!("User {USERNAME} may run the following commands on {hostname}:")
    );
}

#[test]
fn regular_user_can_use_list_regardless_of_which_target_user_is_allowed_by_sudoers() {
    let hostname = "container";
    let env = Env(format!("{USERNAME} ALL=(doesnt:matter) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .hostname(hostname)
        .build();

    let output = Command::new("sudo")
        .args(["-S", "-l"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env);

    assert_contains!(
        output.stdout(),
        format!("User {USERNAME} may run the following commands on {hostname}:")
    );
}

#[test]
fn lists_privileges_for_root() {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).hostname(hostname).build();

    let output = Command::new("sudo").arg("-l").output(&env);

    output.assert_success();

    let expected = format!(
        "User root may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL"
    );
    let actual = output.stdout();
    assert_eq!(strip_matching_defaults_message(&actual), expected);
}

#[test]
fn works_with_long_form_list_flag() {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).hostname(hostname).build();

    let output = Command::new("sudo").arg("--list").output(&env);

    output.assert_success();

    let expected = format!(
        "User root may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL"
    );
    let actual = output.stdout();
    assert_eq!(strip_matching_defaults_message(&actual), expected);
}

#[test]
fn lists_privileges_for_invoking_user_on_current_host() {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .hostname(hostname)
        .build();

    let output = Command::new("sudo")
        .arg("-l")
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();
    assert!(output.stderr().is_empty());

    let expected = format!(
        "User {USERNAME} may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL"
    );
    let actual = output.stdout();
    assert_eq!(strip_matching_defaults_message(&actual), expected);
}

#[test]
fn works_with_uppercase_u_flag() {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .hostname(hostname)
        .build();

    let output = Command::new("sudo")
        .args(["-U", USERNAME, "-l"])
        .output(&env);

    output.assert_success();
    assert!(output.stderr().is_empty());

    let expected = format!(
        "User {USERNAME} may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL"
    );
    let actual = output.stdout();
    assert_eq!(strip_matching_defaults_message(&actual), expected);
}

#[test]
fn fails_with_uppercase_u_flag_when_not_allowed_in_sudoers() {
    let hostname = "container";
    let env = Env("").user(USERNAME).hostname(hostname).build();

    let output = Command::new("sudo")
        .args(["-U", USERNAME, "-l"])
        .output(&env);

    output.assert_success();
    assert!(output.stderr().is_empty());

    let expected = format!("User {USERNAME} is not allowed to run sudo on {hostname}.");
    let actual = output.stdout();
    assert_eq!(actual, expected);
}

#[test]
fn fails_when_user_is_not_allowed_in_sudoers_no_command() {
    let hostname = "container";
    let env = Env("")
        .user(User(USERNAME).password(PASSWORD))
        .hostname(hostname)
        .build();

    let output = Command::new("sudo")
        .args(["-S", "-l"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env);

    output.assert_exit_code(1);

    let diagnostic = format!("Sorry, user {USERNAME} may not run sudo on {hostname}.");
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn does_not_work_with_lowercase_u_flag() {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .hostname(hostname)
        .build();

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "-l"])
        .output(&env);

    assert!(!output.status().success());

    let actual = output.stderr();
    let diagnostic = if sudo_test::is_original_sudo() {
        "usage: sudo -h | -K | -k | -V"
    } else {
        "'--user' flag must be accompanied by a command"
    };
    assert_contains!(actual, diagnostic);
}

#[test]
fn when_specified_multiple_times_uses_longer_format() {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .hostname(hostname)
        .build();

    let output = Command::new("sudo")
        .args(["-l", "-l"])
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();
    assert!(output.stderr().is_empty());

    let expected = format!(
        "User {USERNAME} may run the following commands on {hostname}:\n
Sudoers entry:
    RunAsUsers: ALL
    RunAsGroups: ALL
    Options: !authenticate
    Commands:
\tALL"
    );
    let actual = output.stdout();
    assert_eq!(
        strip_matching_defaults_message(&actual)
            .replace(&format!("Sudoers entry: {ETC_SUDOERS}"), "Sudoers entry:"),
        expected
    );
}

#[test]
fn when_command_is_specified_the_fully_qualified_path_is_displayed() {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: /usr/bin/true")
        .user(USERNAME)
        .build();

    let output = Command::new("sudo")
        .args(["-l", "true"])
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();

    let expected = BIN_TRUE;
    let actual = output.stdout();

    assert_eq!(actual, expected);
}

#[test]
fn when_several_commands_specified_only_first_displayed_with_fully_qualified_path() {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: /usr/bin/true, /bin/ls")
        .user(USERNAME)
        .build();

    let output = Command::new("sudo")
        .args(["-l", "true", "ls"])
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();

    let expected = format!("{BIN_TRUE} ls");
    let actual = output.stdout();

    assert_eq!(actual, expected);
}

#[test]
fn when_command_is_forbidden_exit_with_status_1_no_stderr() {
    let env = Env(format!("ALL ALL=(ALL:ALL) NOPASSWD: {BIN_FALSE}"))
        .user(USERNAME)
        .build();

    let output = Command::new("sudo")
        .args(["-l", "ls"])
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);
    assert!(output.stderr().is_empty());
}

#[test]
fn uppercase_u_flag_matches_on_first_component_of_sudoers_rules() {
    let hostname = "container";
    let env = Env(format!(
        "ALL ALL=({USERNAME}:ALL) {BIN_TRUE}
        {USERNAME} ALL=(ALL:ALL) {BIN_PWD}
        {USERNAME} ALL=(root:ALL) {BIN_FALSE}
        root ALL=(ALL:ALL) {BIN_LS}
        root ALL=({USERNAME}:ALL) /usr/bin/date
        ALL ALL=(root:ALL) /usr/bin/whoami
    "
    ))
    .user(USERNAME)
    .hostname(hostname)
    .build();

    let output = Command::new("sudo")
        .args(["-l", "-U", USERNAME])
        .output(&env);

    output.assert_success();
    assert!(output.stderr().is_empty());

    let expected = format!(
        "User {USERNAME} may run the following commands on {hostname}:
    ({USERNAME} : ALL) {BIN_TRUE}
    (ALL : ALL) {BIN_PWD}
    (root : ALL) {BIN_FALSE}
    (root : ALL) /usr/bin/whoami"
    );
    let actual = output.stdout();
    assert_eq!(strip_matching_defaults_message(&actual), expected);
}

#[test]
fn lowercase_u_flag_matches_users_inside_parenthesis_in_sudoers_rules() {
    let another_user = "another_user";
    let hostname = "container";
    let env = Env(format!(
        "root ALL=({another_user}:ALL)   {BIN_FALSE}
        root ALL=(ALL:ALL)     {BIN_PWD}
        ALL ALL=({another_user}:ALL)    /usr/bin/whoami"
    ))
    .user(another_user)
    .hostname(hostname)
    .build();

    let actual = Command::new("sudo")
        .args(["-l", "-u", another_user, "false", "pwd", "whoami"])
        .output(&env);

    actual.assert_success();
    assert_eq!(format!("{BIN_FALSE} pwd whoami"), actual.stdout());
}

#[test]
fn lowercase_u_flag_not_matching_on_first_component_of_sudoers_rules() {
    let another_user = "another_user";
    let hostname = "container";
    let env = Env(format!(
        "root ALL=ALL
{another_user} ALL=(ALL:ALL) {BIN_LS}"
    ))
    .user(another_user)
    .hostname(hostname)
    .build();

    let actual = Command::new("sudo")
        .args(["-l", "-u", another_user, "ls"])
        .output(&env);

    actual.assert_exit_code(1);
    assert!(actual.stderr().is_empty());
}

#[test]
fn resolves_command_in_invoking_users_path_fail() {
    let env = Env("ALL ALL=(ALL:ALL) ALL").build();

    let output = Command::new("env")
        .args(["-i", "sudo", "-l", "true"])
        .output(&env);

    output.assert_exit_code(1);
    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: true: command not found"
    } else {
        "sudo-rs: 'true': command not found"
    };
    assert_eq!(output.stderr(), diagnostic);
}

#[test]
fn resolves_command_in_invoking_users_path_pass() {
    let expected = "/tmp/true";
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file(expected, TextFile("").chmod("100"))
        .build();

    let output = Command::new("env")
        .args(["-i", "PATH=/tmp", BIN_SUDO, "-l", "true"])
        .output(&env);

    let actual = output.stdout();
    assert_eq!(actual, expected);
}

#[test]
fn relative_path_pass() {
    let prog_abs_path = "/tmp/true";
    let prog_rel_path = "./true";
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file(prog_abs_path, TextFile("").chmod("100"))
        .build();

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("cd /tmp; sudo -l {prog_rel_path}"))
        .output(&env);

    let actual = output.stdout();
    assert_eq!(prog_rel_path, actual);
}

#[test]
fn relative_path_does_not_exist() {
    let prog_rel_path = "./true";
    let env = Env("ALL ALL=(ALL:ALL) ALL").build();

    let output = Command::new("sudo").args(["-l", "./true"]).output(&env);

    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("sudo: {prog_rel_path}: command not found")
    } else {
        format!("sudo-rs: '{prog_rel_path}': command not found")
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn does_not_panic_on_io_errors() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) ALL").build();
    let output = Command::new("bash")
        .args(["-c", "sudo --list | true; echo \"${PIPESTATUS[0]}\""])
        .output(&env);

    let stderr = output.stderr();

    assert!(stderr.is_empty());

    let stdout = output.stdout().parse()?;
    assert_ne!(PANIC_EXIT_CODE, stdout);
    assert_eq!(0, stdout);

    Ok(())
}
