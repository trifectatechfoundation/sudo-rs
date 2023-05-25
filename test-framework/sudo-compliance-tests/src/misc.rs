use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "snapshots/misc",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn user_not_in_passwd_database_cannot_use_sudo() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user_id(1000)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "user `current user' not found");
    }

    Ok(())
}

#[test]
#[ignore]
fn closes_open_file_descriptors() -> Result<()> {
    let script_path = "/tmp/script.bash";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(
            script_path,
            include_str!("misc/read-parents-open-file-descriptor.bash"),
        )
        .build()?;

    let output = Command::new("bash").arg(script_path).exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    assert_contains!(output.stderr(), "42: Bad file descriptor");

    Ok(())
}

#[test]
#[ignore]
fn sudo_binary_lacks_setuid_flag() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    Command::new("chmod")
        .args(["0755", "/usr/bin/sudo"])
        .exec(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    assert_contains!(
        output.stderr(),
        "sudo must be owned by uid 0 and have the setuid bit set"
    );

    Ok(())
}

#[test]
#[ignore]
fn sudo_binary_is_not_owned_by_root() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    Command::new("chown")
        .args([USERNAME, "/usr/bin/sudo"])
        .exec(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    assert_contains!(
        output.stderr(),
        "sudo must be owned by uid 0 and have the setuid bit set"
    );

    Ok(())
}
