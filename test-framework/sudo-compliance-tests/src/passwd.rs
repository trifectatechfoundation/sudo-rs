use sudo_test::{Command, Env};

use crate::{Result, USERNAME, SUDOERS_NO_LECTURE};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![(r"[[:xdigit:]]{12}", "[host]")],
            prepend_module_to_snapshot => false,
            snapshot_path => "snapshots/passwd",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn explicit_passwd_overrides_nopasswd() -> Result<()> {
    let env = Env([
        "ALL ALL=(ALL:ALL) NOPASSWD: /usr/bin/true, PASSWD: /usr/bin/ls",
        SUDOERS_NO_LECTURE,
    ])
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?;

        assert!(output.status().success());

    let second_output = Command::new("sudo")
        .args(["-S", "ls"])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!second_output.status().success());
    assert_eq!(Some(1), second_output.status().code());

    let stderr = second_output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "Maximum 3 incorrect authentication attempts"
        );
    }

    Ok(())
}
