use sudo_test::{Command, Env};

use crate::{Result, LONGEST_HOSTNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/sudoers/host_list",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

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

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
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

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
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

#[test]
fn longest_hostname() -> Result<()> {
    let env = Env(format!("ALL {LONGEST_HOSTNAME} = (ALL:ALL) ALL"))
        .hostname(LONGEST_HOSTNAME)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}
