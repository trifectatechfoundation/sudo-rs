use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

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

    let expected = format!("User root may run the following commands on {hostname}:\n    (ALL : ALL) NOPASSWD: ALL");
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

    let expected = format!("User {USERNAME} may run the following commands on {hostname}:\n    (ALL : ALL) NOPASSWD: ALL");
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

    let expected = format!("User {USERNAME} may run the following commands on {hostname}:\n    (ALL : ALL) NOPASSWD: ALL");
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

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

    let expected = format!("User {USERNAME} may run the following commands on {hostname}:\n\nSudoers entry:\n    RunAsUsers: ALL\n    RunAsGroups: ALL\n    Options: !authenticate\n    Commands:\n\tALL");
    let actual = output.stdout()?;
    assert_eq!(actual, expected);

    Ok(())
}

#[ignore = "gh658"]
#[test]
fn when_command_is_specified_the_fully_qualified_path_is_displayed() -> Result<()> {
    let env = Env(format!("ALL ALL=(ALL:ALL) NOPASSWD: /bin/true"))
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo")
    .args(["-l", "true"])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success());

    let expected = format!("/usr/bin/true");
    let actual = output.stdout()?;

    assert_eq!(actual, expected);

    Ok(())
}

#[ignore = "gh658"]
#[test]
fn when_command_is_forbidden_exit_with_status_1_no_stderr() -> Result<()> {
    let env = Env(format!("ALL ALL=(ALL:ALL) NOPASSWD: /bin/true"))
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
