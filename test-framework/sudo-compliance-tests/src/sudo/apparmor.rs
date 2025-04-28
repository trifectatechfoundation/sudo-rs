use sudo_test::{Command, Env};

use crate::{Result, PANIC_EXIT_CODE};

#[cfg(feature = "apparmor")]
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
        .build()?;

    let output = Command::new("bash")
        .args(["-c", r#"sudo bash -c "echo \$\$""#])
        .output(&env)?;
    dbg!(&output);

    let exit_code = output.stdout()?.parse()?;
    assert_ne!(PANIC_EXIT_CODE, exit_code);
    assert_eq!(0, exit_code);

    Ok(())
}
