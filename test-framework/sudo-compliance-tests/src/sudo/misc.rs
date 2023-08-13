use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/misc",
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
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "user 'current user' not found");
    }

    Ok(())
}

fn closes_open_file_descriptors(tty: bool) -> Result<()> {
    let script_path = "/tmp/script.bash";
    let defaults = if tty {
        "Defaults use_pty"
    } else {
        "Defaults !use_pty"
    };
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, defaults])
        .file(
            script_path,
            include_str!("misc/read-parents-open-file-descriptor.bash"),
        )
        .build()?;

    let output = Command::new("bash")
        .arg(script_path)
        .tty(tty)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    assert_contains!(
        if tty {
            // Docker merges stderr into stdout with "--tty". See gh622
            output.stdout_unchecked()
        } else {
            output.stderr()
        },
        "42: Bad file descriptor"
    );

    Ok(())
}

#[test]
fn closes_open_file_descriptors_with_tty() -> Result<()> {
    closes_open_file_descriptors(true)
}

#[test]
fn closes_open_file_descriptors_without_tty() -> Result<()> {
    closes_open_file_descriptors(false)
}

#[test]
fn sudo_binary_lacks_setuid_flag() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    Command::new("chmod")
        .args(["0755", "/usr/bin/sudo"])
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    assert_contains!(
        output.stderr(),
        "sudo must be owned by uid 0 and have the setuid bit set"
    );

    Ok(())
}

#[test]
fn sudo_binary_is_not_owned_by_root() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    Command::new("chown")
        .args([USERNAME, "/usr/bin/sudo"])
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    assert_contains!(
        output.stderr(),
        "sudo must be owned by uid 0 and have the setuid bit set"
    );

    Ok(())
}

#[test]
fn works_when_invoked_through_a_symlink() -> Result<()> {
    let symlink_path = "/tmp/sudo";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    Command::new("ln")
        .args(["-s", "/usr/bin/sudo", symlink_path])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    // symlink is not owned by root
    let ls_output = Command::new("ls")
        .args(["-ahl", symlink_path])
        .output(&env)?
        .stdout()?;

    // lrwxrwxrwx 1 ferris users
    eprintln!("{ls_output}");

    // symlink has not the setuid bit set
    let stat_output = Command::new("stat")
        .args(["-c", "%a", symlink_path])
        .output(&env)?
        .stdout()?;

    // 777
    eprintln!("{stat_output}");

    // still, we expect sudo to work because the executable behind the symlink has the right
    // ownership and permissions
    Command::new(symlink_path)
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}
