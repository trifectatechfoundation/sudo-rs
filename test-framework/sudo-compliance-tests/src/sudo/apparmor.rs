use sudo_test::{Command, Env};

use crate::Result;

#[cfg(feature = "apparmor")]
//TODO: the apparmor profile needs to be present in the *host* machine.
//This can't be done automatically in the test framework. See issue #1094
#[test]
fn switches_the_apparmor_profile() -> Result<()> {
    let env = Env("root ALL=(ALL:ALL) APPARMOR_PROFILE=sudo_test ALL")
        .file(
            "/etc/apparmor.d/sudo_test",
            r#"
            abi <abi/3.0>,

            include <tunables/global>

            profile sudo_test {
                include <abstractions/base>

                owner @{PROC}/@{pid}/attr/current r,

                /dev/tty rw,

                /usr/bin/cat ixr,
            }
        "#,
        )
        .build();

    let output = Command::new("bash")
        .args(["-c", r#"sudo bash -c "echo \$\$""#])
        .output(&env);
    dbg!(&output);

    output.assert_success();

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
