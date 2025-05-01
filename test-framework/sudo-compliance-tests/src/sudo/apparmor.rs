use sudo_test::{Command, Env};

use crate::Result;

#[test]
fn switches_the_apparmor_profile() -> Result<()> {
    let env = Env("root ALL=(ALL:ALL) APPARMOR_PROFILE=docker-default ALL")
        .apparmor("unconfined")
        .build();

    let output = Command::new("sudo")
        .args(["-s", "cat", "/proc/$$/attr/current"])
        .output(&env);
    dbg!(&output);

    output.assert_success();
    assert_eq!(output.stdout(), "docker-default (enforce)");

    Ok(())
}

#[test]
fn cannot_switch_to_nonexisting_profile() -> Result<()> {
    let env = Env("root ALL=(ALL:ALL) APPARMOR_PROFILE=this_profile_does_not_exist ALL").build();

    let output = Command::new("sudo").arg("true").output(&env);

    dbg!(&output);

    output.assert_exit_code(1);
    assert_contains!(output.stderr(), "unable to change AppArmor profile");

    Ok(())
}
