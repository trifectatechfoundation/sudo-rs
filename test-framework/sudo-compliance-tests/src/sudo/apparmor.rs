use sudo_test::{Command, Env};

use crate::Result;

#[test]
fn can_affirm_the_apparmor_profile() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) APPARMOR_PROFILE=unconfined NOPASSWD: ALL")
        .apparmor("unconfined")
        .build();

    let output = Command::new("sudo")
        .args(["-s", "cat", "/proc/$$/attr/current"])
        .output(&env);

    output.assert_success();
    assert_eq!(output.stdout().as_str(), "unconfined");

    Ok(())
}

#[test]
fn can_switch_the_apparmor_profile() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) APPARMOR_PROFILE=docker-default NOPASSWD: ALL")
        .apparmor("unconfined")
        .build();

    let output = Command::new("sudo")
        .args(["-s", "cat", "/proc/$$/attr/current"])
        .output(&env);

    output.assert_success();
    assert!(matches!(
        output.stdout().as_str(),
        "docker-default (enforce)" | "docker-default//&unconfined (enforce)"
    ));

    Ok(())
}

#[test]
fn cannot_switch_to_nonexisting_profile() -> Result<()> {
    let env = Env("root ALL=(ALL:ALL) APPARMOR_PROFILE=this_profile_does_not_exist NOPASSWD: ALL")
        .build();

    let output = Command::new("sudo").arg("true").output(&env);

    output.assert_exit_code(1);
    assert_contains!(output.stderr(), "unable to change AppArmor profile");

    Ok(())
}

#[test]
fn can_set_default_apparmor_profile() -> Result<()> {
    let env = Env("root ALL=(ALL:ALL) NOPASSWD: ALL
Defaults apparmor_profile=docker-default
")
    .apparmor("unconfined")
    .build();

    let output = Command::new("sudo")
        .args(["-s", "cat", "/proc/$$/attr/current"])
        .output(&env);

    output.assert_success();
    assert!(matches!(
        output.stdout().as_str(),
        "docker-default (enforce)" | "docker-default//&unconfined (enforce)"
    ));

    Ok(())
}

#[test]
fn tags_override_the_default_apparmor_profile() -> Result<()> {
    let env = Env(
        "root ALL=(ALL:ALL) APPARMOR_PROFILE=unconfined NOPASSWD: ALL
Defaults apparmor_profile=docker-default
",
    )
    .apparmor("unconfined")
    .build();

    let output = Command::new("sudo")
        .args(["-s", "cat", "/proc/$$/attr/current"])
        .output(&env);

    output.assert_success();
    assert_eq!(output.stdout(), "unconfined");

    Ok(())
}
