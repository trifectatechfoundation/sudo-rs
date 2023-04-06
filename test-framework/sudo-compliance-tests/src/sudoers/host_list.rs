use sudo_test::{Command, Env};

use crate::Result;

#[test]
fn given_specific_hostname_then_sudo_from_said_hostname_is_allowed() -> Result<()> {
    let hostname = "container";
    let env = Env(format!("ALL {hostname} = (ALL:ALL) ALL"))
        .hostname(hostname)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}

#[test]
fn given_specific_hostname_then_sudo_from_different_hostname_is_rejected() -> Result<()> {
    let env = Env("ALL remotehost = (ALL:ALL) ALL")
        .hostname("container")
        .build()?;

    let output = Command::new("sudo").arg("true").exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        insta::assert_snapshot!(output.stderr());
    }

    Ok(())
}

#[test]
fn different() -> Result<()> {
    let env = Env("ALL remotehost, container = (ALL:ALL) ALL")
        .hostname("container")
        .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}

#[test]
fn repeated() -> Result<()> {
    let env = Env("ALL container, container = (ALL:ALL) ALL")
        .hostname("container")
        .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}

#[test]
fn negation_rejects() -> Result<()> {
    let env = Env("ALL remotehost, !container = (ALL:ALL) ALL")
        .hostname("container")
        .build()?;

    let output = Command::new("sudo").arg("true").exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        insta::assert_snapshot!(output.stderr());
    }

    Ok(())
}

#[test]
fn double_negative_is_positive() -> Result<()> {
    let env = Env("ALL !!container = (ALL:ALL) ALL")
        .hostname("container")
        .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}
