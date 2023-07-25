use sudo_test::{Command, Env, User};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME, PASSWORD};

#[ignore = "gh658"]
#[test]
fn lists_privileges_for_root() -> Result<()> {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .hostname(hostname)
        .build()?;

    let output = Command::new("sudo")
        .arg("-l")
        .output(&env)?;

    assert!(output.status().success());

    let expected = format!("User root may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL");
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[ignore = "gh658"]
#[test]
fn works_with_long_form_list_flag() -> Result<()> {
    let hostname = "container";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .hostname(hostname)
        .build()?;

    let output = Command::new("sudo")
        .arg("--list")
        .output(&env)?;

    assert!(output.status().success());

    let expected = format!("User root may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL");
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[ignore = "gh658"]
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

    let expected = format!("User {USERNAME} may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL");
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[ignore = "gh658"]
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

    let expected = format!("User {USERNAME} may run the following commands on {hostname}:
    (ALL : ALL) NOPASSWD: ALL");
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[ignore = "gh658"]
#[test]
fn fails_with_uppercase_u_flag_when_not_allowed_in_sudoers() -> Result<()> {
    let hostname = "container";
    let env = Env("")
        .user(USERNAME)
        .hostname(hostname)
        .build()?;

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

#[ignore = "gh658"]
#[test]
fn fails_when_user_is_not_allowed_in_sudoers() -> Result<()> {
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

    let expected = format!("password for {USERNAME}: Sorry, user {USERNAME} may not run sudo on {hostname}.");
    let actual = output.stderr();
    assert_contains!(actual, expected);

    Ok(())
}

#[ignore = "gh658"]
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
    assert_contains!(actual, "usage: sudo -h | -K | -k | -V");

    Ok(())
}

#[ignore = "gh658"]
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

    let expected = format!("User {USERNAME} may run the following commands on {hostname}:\n
Sudoers entry:
    RunAsUsers: ALL
    RunAsGroups: ALL
    Options: !authenticate
    Commands:
\tALL");
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[ignore = "gh658"]
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

#[ignore = "gh658"]
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

#[ignore = "gh658"]
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

#[ignore = "gh658"]
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
    "))
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
    (root : ALL) /usr/bin/whoami");
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[ignore = "gh658"]
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

#[ignore = "gh658"]
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
