//! Test the run_as component of the user specification: <user> ALL=(<run_as>) ALL`

use sudo_test::{Command, Env, User};

use crate::{Result, GROUPNAME, PAMD_SUDO_PAM_PERMIT, SUDOERS_NO_LECTURE, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![(r"[[:xdigit:]]{12}", "[host]")],
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/sudoers/run_as",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

// "If both Runas_Lists are empty, the command may only be run as the invoking user."
#[test]
#[ignore]
fn when_empty_then_implicit_as_self_is_allowed() -> Result<()> {
    let env = Env("ALL ALL=() NOPASSWD: ALL").user(USERNAME).build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_empty_then_explicit_as_self_is_allowed() -> Result<()> {
    let env = Env("ALL ALL=() NOPASSWD: ALL").user(USERNAME).build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-u", user, "true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_empty_then_as_someone_else_is_not_allowed() -> Result<()> {
    let env = Env("ALL ALL=() NOPASSWD: ALL").user(USERNAME).build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "true"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_snapshot!(output.stderr());
    }

    Ok(())
}

#[test]
fn when_empty_then_as_own_group_is_allowed() -> Result<()> {
    let env = Env("ALL ALL=() NOPASSWD: ALL")
        .group(USERNAME)
        .user(User(USERNAME).secondary_group(USERNAME))
        .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-g", user, "true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_specific_user_then_as_that_user_is_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=({USERNAME}) NOPASSWD: ALL"))
        .user(USERNAME)
        .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-u", USERNAME, "true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_specific_user_then_as_a_different_user_is_not_allowed() -> Result<()> {
    let env = Env("ALL ALL=(ferris) NOPASSWD: ALL")
        .user("ferris")
        .user("ghost")
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", "ghost", "true"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_snapshot!(output.stderr());
    }

    Ok(())
}

#[test]
fn when_specific_user_then_as_self_is_not_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=({USERNAME}) NOPASSWD: ALL")).build()?;

    let output = Command::new("sudo").args(["true"]).exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_snapshot!(output.stderr());
    }

    Ok(())
}

// "If only the first is specified, the command may be run as any user in the list but no -g option
// may be specified."
// NOTE: sudoers does not support this yet - questionable whether we want to copy
// this behaviour in the case that a user specifies its own group
// however, this test case passes (i.e. the command fails) due to user not being in group
#[test]
fn when_only_user_is_specified_then_group_flag_is_not_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=({USERNAME}) ALL"))
        // NOPASSWD does not seem to apply to the regular user so use PAM to avoid password input
        .file("/etc/pam.d/sudo", PAMD_SUDO_PAM_PERMIT)
        .user(USERNAME)
        .group(GROUPNAME)
        .build()?;

    for user in ["root", USERNAME] {
        let output = Command::new("sudo")
            .args(["-g", GROUPNAME, "true"])
            .as_user(user)
            .exec(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        if sudo_test::is_original_sudo() {
            assert_contains!(
                output.stderr(),
                " is not allowed to execute '/bin/true' as "
            );
        }
    }

    Ok(())
}

#[test]
fn when_specific_group_then_as_that_group_is_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=(:{GROUPNAME}) NOPASSWD: ALL"))
        .user(USERNAME)
        .group(GROUPNAME)
        .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-g", GROUPNAME, "true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_specific_group_then_as_a_different_group_is_not_allowed() -> Result<()> {
    let env = Env([&format!("ALL ALL=(:{GROUPNAME})  ALL"), SUDOERS_NO_LECTURE])
        // NOPASSWD does not seem to apply to the regular user so use PAM to avoid password input
        .file("/etc/pam.d/sudo", PAMD_SUDO_PAM_PERMIT)
        .user(USERNAME)
        .group(GROUPNAME)
        .group("ghosts")
        .build()?;

    for user in ["root", USERNAME] {
        let output = Command::new("sudo")
            .args(["-g", "ghosts", "true"])
            .as_user(user)
            .exec(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        if sudo_test::is_original_sudo() {
            assert_snapshot!(output.stderr());
        }
    }

    Ok(())
}

#[test]
fn when_only_group_is_specified_then_as_some_user_is_not_allowed() -> Result<()> {
    let env = Env([&format!("ALL ALL=(:{GROUPNAME})  ALL"), SUDOERS_NO_LECTURE])
        // NOPASSWD does not seem to apply to the regular user so use PAM to avoid password input
        .file("/etc/pam.d/sudo", PAMD_SUDO_PAM_PERMIT)
        .user(USERNAME)
        .user("ghost")
        .group(GROUPNAME)
        .build()?;

    for user in ["root", USERNAME] {
        let output = Command::new("sudo")
            .args(["-u", "ghost", "true"])
            .as_user(user)
            .exec(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        if sudo_test::is_original_sudo() {
            assert_snapshot!(output.stderr());
        }
    }

    Ok(())
}

// "If both Runas_Lists are specified, the command may be run with any combination of users and
// groups listed in their respective Runas_Lists."
#[test]
fn when_both_user_and_group_are_specified_then_as_that_user_is_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=({USERNAME}:{GROUPNAME}) NOPASSWD: ALL"))
        .user(USERNAME)
        .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-u", USERNAME, "true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
#[ignore]
fn when_both_user_and_group_are_specified_then_as_that_group_is_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=({USERNAME}:{GROUPNAME}) NOPASSWD: ALL"))
        .user(USERNAME)
        .group(GROUPNAME)
        .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-g", GROUPNAME, "true"])
            .as_user(user)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}
