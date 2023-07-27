use sudo_test::{Command, Env, TextFile, User};

use crate::{Result, PASSWORD, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

mod credential_caching;
mod flag_other_user;
mod needs_auth;
mod nopasswd;
mod not_allowed;
mod short_format;
mod sudoers_list;

#[test]
fn root_cannot_use_list_when_empty_sudoers() -> Result<()> {
    let hostname = "container";
    let env = Env("").hostname(hostname).build()?;

    let output = Command::new("sudo").arg("-l").output(&env)?;

    assert!(output.status().success());

    let expected = format!("User root is not allowed to run sudo on {hostname}.");
    let actual = output.stdout()?;
    assert_contains!(actual, expected);

    Ok(())
}

#[test]
fn regular_user_can_use_list_regardless_of_which_command_is_allowed_by_sudoers() -> Result<()> {
    let hostname = "container";
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) /command/does/not/matter"))
        .user(User(USERNAME).password(PASSWORD))
        .hostname(hostname)
        .build()?;

    let output = Command::new("sudo")
        .args(["-S", "-l"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?;

    assert_contains!(
        output.stdout()?,
        format!("User {USERNAME} may run the following commands on {hostname}:")
    );

    Ok(())
}

#[test]
fn regular_user_can_use_list_regardless_of_which_target_user_is_allowed_by_sudoers() -> Result<()> {
    let hostname = "container";
    let env = Env(format!("{USERNAME} ALL=(doesnt:matter) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .hostname(hostname)
        .build()?;

    let output = Command::new("sudo")
        .args(["-S", "-l"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?;

    assert_contains!(
        output.stdout()?,
        format!("User {USERNAME} may run the following commands on {hostname}:")
    );

    Ok(())
}

#[test]
fn lists_privileges_for_root() -> Result<()> {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).hostname(hostname).build()?;

    let output = Command::new("sudo").arg("-l").output(&env)?;

    assert!(output.status().success());

    let expected = format!(
        "User root may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL"
    );
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[test]
fn works_with_long_form_list_flag() -> Result<()> {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).hostname(hostname).build()?;

    let output = Command::new("sudo").arg("--list").output(&env)?;

    assert!(output.status().success());

    let expected = format!(
        "User root may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL"
    );
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[test]
fn lists_privileges_for_invoking_user_on_current_host() -> Result<()> {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .hostname(hostname)
        .build()?;

    let output = Command::new("sudo")
        .arg("-l")
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success());
    assert!(output.stderr().is_empty());

    let expected = format!(
        "User {USERNAME} may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL"
    );
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[test]
fn works_with_uppercase_u_flag() -> Result<()> {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .hostname(hostname)
        .build()?;

    let output = Command::new("sudo")
        .args(["-U", USERNAME, "-l"])
        .output(&env)?;

    assert!(output.status().success());
    assert!(output.stderr().is_empty());

    let expected = format!(
        "User {USERNAME} may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL"
    );
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[test]
fn fails_with_uppercase_u_flag_when_not_allowed_in_sudoers() -> Result<()> {
    let hostname = "container";
    let env = Env("").user(USERNAME).hostname(hostname).build()?;

    let output = Command::new("sudo")
        .args(["-U", USERNAME, "-l"])
        .output(&env)?;

    assert!(output.status().success());
    assert!(output.stderr().is_empty());
    assert_eq!(Some(0), output.status().code());

    let expected = format!("User {USERNAME} is not allowed to run sudo on {hostname}.");
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[test]
fn fails_when_user_is_not_allowed_in_sudoers_no_command() -> Result<()> {
    let hostname = "container";
    let env = Env("")
        .user(User(USERNAME).password(PASSWORD))
        .hostname(hostname)
        .build()?;

    let output = Command::new("sudo")
        .args(["-S", "-l"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = format!("Sorry, user {USERNAME} may not run sudo on {hostname}.");
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn does_not_work_with_lowercase_u_flag() -> Result<()> {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .hostname(hostname)
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "-l"])
        .output(&env)?;

    assert!(!output.status().success());

    let actual = output.stderr();
    let diagnostic = if sudo_test::is_original_sudo() {
        "usage: sudo -h | -K | -k | -V"
    } else {
        "invalid argument found for '--list"
    };
    assert_contains!(actual, diagnostic);

    Ok(())
}

#[ignore = "gh710"]
#[test]
fn when_specified_multiple_times_uses_longer_format() -> Result<()> {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .hostname(hostname)
        .build()?;

    let output = Command::new("sudo")
        .args(["-l", "-l"])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success());
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
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[test]
fn when_command_is_specified_the_fully_qualified_path_is_displayed() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: /bin/true")
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo")
        .args(["-l", "true"])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success());

    let expected = "/usr/bin/true";
    let actual = output.stdout()?;

    assert_eq!(actual, expected);

    Ok(())
}

#[test]
fn when_several_commands_specified_only_first_displayed_with_fully_qualified_path() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: /bin/true, /bin/ls")
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo")
        .args(["-l", "true", "ls"])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success());

    let expected = "/usr/bin/true ls";
    let actual = output.stdout()?;

    assert_eq!(actual, expected);

    Ok(())
}

#[test]
fn when_command_is_forbidden_exit_with_status_1_no_stderr() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: /bin/true")
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo")
        .args(["-l", "ls"])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert!(output.stderr().is_empty());

    Ok(())
}

#[test]
fn uppercase_u_flag_matches_on_first_component_of_sudoers_rules() -> Result<()> {
    let hostname = "container";
    let env = Env(format!(
        "ALL ALL=({USERNAME}:ALL) /usr/bin/true
        {USERNAME} ALL=(ALL:ALL) /usr/bin/pwd
        {USERNAME} ALL=(root:ALL) /usr/bin/false
        root ALL=(ALL:ALL) /usr/bin/ls
        root ALL=({USERNAME}:ALL) /usr/bin/date
        ALL ALL=(root:ALL) /usr/bin/whoami
    "
    ))
    .user(USERNAME)
    .hostname(hostname)
    .build()?;

    let output = Command::new("sudo")
        .args(["-l", "-U", USERNAME])
        .output(&env)?;

    assert!(output.status().success());
    assert!(output.stderr().is_empty());

    let expected = format!(
        "User {USERNAME} may run the following commands on {hostname}:
    ({USERNAME} : ALL) /usr/bin/true
    (ALL : ALL) /usr/bin/pwd
    (root : ALL) /usr/bin/false
    (root : ALL) /usr/bin/whoami"
    );
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[test]
fn lowercase_u_flag_matches_users_inside_parenthesis_in_sudoers_rules() -> Result<()> {
    let another_user = "another_user";
    let hostname = "container";
    let env = Env(format!(
        "root ALL=({another_user}:ALL)   /usr/bin/false
        root ALL=(ALL:ALL)     /usr/bin/pwd
        ALL ALL=({another_user}:ALL)    /usr/bin/whoami"
    ))
    .user(another_user)
    .hostname(hostname)
    .build()?;

    let actual = Command::new("sudo")
        .args(["-l", "-u", another_user, "false", "pwd", "whoami"])
        .output(&env)?;

    assert!(actual.status().success());
    assert_eq!("/usr/bin/false pwd whoami", actual.stdout()?);

    Ok(())
}

#[test]
fn lowercase_u_flag_not_matching_on_first_component_of_sudoers_rules() -> Result<()> {
    let another_user = "another_user";
    let hostname = "container";
    let env = Env(format!("{another_user} ALL=(ALL:ALL) /usr/bin/ls"))
        .user(another_user)
        .hostname(hostname)
        .build()?;

    let actual = Command::new("sudo")
        .args(["-l", "-u", another_user, "ls"])
        .output(&env)?;

    assert!(!actual.status().success());
    assert_eq!(Some(1), actual.status().code());
    assert!(actual.stderr().is_empty());

    Ok(())
}

#[test]
fn resolves_command_in_invoking_users_path_fail() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) ALL").build()?;

    let output = Command::new("env")
        .args(["-i", "sudo", "-l", "true"])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: true: command not found"
    } else {
        "sudo-rs: 'true': command not found"
    };
    assert_eq!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn resolves_command_in_invoking_users_path_pass() -> Result<()> {
    let expected = "/tmp/true";
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file(expected, TextFile("").chmod("100"))
        .build()?;

    let output = Command::new("env")
        .args(["-i", "PATH=/tmp", "/usr/bin/sudo", "-l", "true"])
        .output(&env)?;

    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[test]
fn relative_path_pass() -> Result<()> {
    let prog_abs_path = "/tmp/true";
    let prog_rel_path = "./true";
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file(prog_abs_path, TextFile("").chmod("100"))
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("cd /tmp; sudo -l {prog_rel_path}"))
        .output(&env)?;

    let actual = output.stdout()?;
    assert_eq!(prog_rel_path, actual);

    Ok(())
}

#[test]
fn relative_path_does_not_exist() -> Result<()> {
    let prog_rel_path = "./true";
    let env = Env("ALL ALL=(ALL:ALL) ALL").build()?;

    let output = Command::new("sudo").args(["-l", "./true"]).output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("sudo: {prog_rel_path}: command not found")
    } else {
        format!("sudo-rs: '{prog_rel_path}': command not found")
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}
