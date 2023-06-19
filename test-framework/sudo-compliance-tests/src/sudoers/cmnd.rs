//! Test the Cmnd_Spec component of the user specification: <user> ALL=(ALL:ALL) <cmnd_spec>

use pretty_assertions::assert_eq;
use sudo_test::{Command, Env, TextFile};

use crate::{Result, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![(r"[[:xdigit:]]{12}", "[host]")],
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/sudoers/cmnd",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn given_specific_command_then_that_command_is_allowed() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /bin/true").build()?;

    Command::new("sudo")
        .arg("/bin/true")
        .output(&env)?
        .assert_success()
}

#[test]
fn given_specific_command_then_other_command_is_not_allowed() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /bin/ls").build()?;

    let output = Command::new("sudo").arg("/bin/true").output(&env)?;

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
fn given_specific_command_with_nopasswd_tag_then_no_password_auth_is_required() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: /bin/true")
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .arg("/bin/true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn command_specified_not_by_absolute_path_is_rejected() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) true").build()?;

    let output = Command::new("sudo").arg("/bin/true").output(&env)?;

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
    let env = Env("ALL ALL=(ALL:ALL) /bin/true, /bin/ls").build()?;

    Command::new("sudo")
        .arg("/bin/true")
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .args(["/bin/ls", "/root"])
        .output(&env)?
        .stdout()?;

    assert_eq!("", output);

    Ok(())
}

// it applies not only to the command is next to but to all commands that follow
#[test]
fn nopasswd_is_sticky() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: /bin/ls, /bin/true")
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .arg("/bin/true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn repeated() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /bin/true, /bin/true").build()?;

    Command::new("sudo")
        .arg("/bin/true")
        .output(&env)?
        .assert_success()
}

#[test]
fn nopasswd_override() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /bin/true, NOPASSWD: /bin/true")
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .arg("/bin/true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn runas_override() -> Result<()> {
    let env = Env(format!("ALL ALL = (root) /bin/ls, ({USERNAME}) /bin/true"))
        .user(USERNAME)
        .build()?;

    let stdout = Command::new("sudo")
        .args(["/bin/ls", "/root"])
        .output(&env)?
        .stdout()?;

    assert_eq!("", stdout);

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "/bin/ls"])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "user root is not allowed to execute '/bin/ls' as ferris"
    } else {
        "authentication failed: I'm sorry root. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);

    Command::new("sudo")
        .args(["-u", "ferris", "/bin/true"])
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo").args(["/bin/true"]).output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "user root is not allowed to execute '/bin/true' as root"
    } else {
        "authentication failed: I'm sorry root. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn runas_override_repeated_cmnd_means_runas_union() -> Result<()> {
    let env = Env(format!(
        "ALL ALL = (root) /usr/bin/true, ({USERNAME}) /usr/bin/true"
    ))
    .user(USERNAME)
    .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()?;

    Command::new("sudo")
        .args(["-u", USERNAME, "true"])
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn given_directory_then_commands_in_it_are_allowed() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /usr/bin/").build()?;

    Command::new("sudo")
        .arg("/usr/bin/true")
        .output(&env)?
        .assert_success()
}

#[test]
fn given_directory_then_commands_in_its_subdirectories_are_not_allowed() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /usr/").build()?;

    let output = Command::new("sudo").arg("/usr/bin/true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "user root is not allowed to execute '/usr/bin/true' as root"
    } else {
        "authentication failed: I'm sorry root. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn wildcards_are_allowed_for_dir() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /usr/*/true").build()?;

    Command::new("sudo")
        .arg("/usr/bin/true")
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn wildcards_are_allowed_for_file() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /usr/bin/*").build()?;

    Command::new("sudo")
        .arg("/usr/bin/true")
        .output(&env)?
        .assert_success()?;

    Ok(())
}

// due to frequent misusage ("sudo: you are doing it wrong"), we explicitly don't support this
#[test]
#[ignore = "wontfix"]
fn wildcards_are_allowed_for_args() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /usr/bin/true /root/*").build()?;

    Command::new("sudo")
        .arg("true")
        .arg("/root/ hello world")
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn arguments_can_be_supplied() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /usr/bin/true").build()?;

    Command::new("sudo")
        .arg("true")
        .arg("/root/ hello world")
        .output(&env)?
        .assert_success()?;

    Command::new("sudo")
        .arg("true")
        .arg("foo")
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn arguments_can_be_forced() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /usr/bin/true hello").build()?;

    Command::new("sudo")
        .arg("true")
        .arg("hello")
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
        .arg("/root/ hello world")
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "user root is not allowed to execute '/usr/bin/true /root/ hello world' as root"
    } else {
        "authentication failed: I'm sorry root. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn arguments_can_be_forbidded() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /usr/bin/true \"\"").build()?;

    let output = Command::new("sudo")
        .arg("true")
        .arg("/root/ hello world")
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "user root is not allowed to execute '/usr/bin/true /root/ hello world' as root"
    } else {
        "authentication failed: I'm sorry root. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn wildcards_dont_cross_directory_boundaries() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) /usr/*/foo")
        .directory("/usr/bin/sub")
        .file("/usr/bin/sub/foo", TextFile("").chown("root").chmod("777"))
        .build()?;

    let output = Command::new("sudo").arg("/usr/bin/sub/foo").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "user root is not allowed to execute '/usr/bin/sub/foo' as root"
    } else {
        "authentication failed: I'm sorry root. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}
