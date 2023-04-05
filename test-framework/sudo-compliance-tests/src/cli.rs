use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD};

#[test]
fn just_dash_dash_works() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    Command::new("sudo")
        .args(["--", "true"])
        .exec(&env)?
        .assert_success()
}

#[test]
fn dash_dash_after_other_flag_works() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    Command::new("sudo")
        .args(["-u", "root", "--", "true"])
        .exec(&env)?
        .assert_success()
}

#[test]
fn dash_dash_before_flag_is_an_error() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let output = Command::new("sudo")
        .args(["--", "-u", "root", "true"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        insta::assert_snapshot!(output.stderr());
    }

    Ok(())
}

#[test]
fn dash_flag_space_value_syntax() -> Result<()> {
    let expected = 0;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let actual = Command::new("sudo")
        .args(["-u", "root", "id", "-u"])
        .exec(&env)?
        .stdout()?
        .parse::<u16>()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn dash_flag_no_space_value_syntax() -> Result<()> {
    let expected = 0;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let actual = Command::new("sudo")
        .args(["-uroot", "id", "-u"])
        .exec(&env)?
        .stdout()?
        .parse::<u16>()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
#[ignore]
fn dash_flag_equal_value_invalid_syntax() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let output = Command::new("sudo")
        .args(["-u=root", "id", "-u"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "sudo: unknown user: =root");
    }

    Ok(())
}

#[test]
fn dash_dash_flag_space_value_syntax() -> Result<()> {
    let expected = 0;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let actual = Command::new("sudo")
        .args(["--user", "root", "id", "-u"])
        .exec(&env)?
        .stdout()?
        .parse::<u16>()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn dash_dash_flag_equal_value_syntax() -> Result<()> {
    let expected = 0;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let actual = Command::new("sudo")
        .args(["--user=root", "id", "-u"])
        .exec(&env)?
        .stdout()?
        .parse::<u16>()?;

    assert_eq!(expected, actual);

    Ok(())
}
