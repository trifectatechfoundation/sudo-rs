//! Test the run_as component of the user specification: <user> ALL=(<run_as>) ALL`

use std::collections::HashSet;

use sudo_test::{Command, Env, User, BIN_TRUE, ROOT_GROUP};

use crate::{Result, GROUPNAME, PAMD_SUDO_PAM_PERMIT, SUDOERS_NO_LECTURE, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![(r"[[:xdigit:]]{12}", "[host]")],
            prepend_module_to_snapshot => false,
            snapshot_path => "../../snapshots/sudoers/run_as",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

// "If both Runas_Lists are empty, the command may only be run as the invoking user."
#[test]
#[ignore = "gh134"]
fn when_empty_then_implicit_as_self_is_allowed() -> Result<()> {
    let env = Env("ALL ALL=() NOPASSWD: ALL").user(USERNAME).build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["true"])
            .as_user(user)
            .output(&env)?
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
            .output(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_empty_then_as_someone_else_is_not_allowed() -> Result<()> {
    let env = Env("ALL ALL=() NOPASSWD: ALL").user(USERNAME).build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "true"])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "I'm sorry root. I'm afraid I can't do that");
    }

    Ok(())
}

#[test]
fn when_empty_then_as_own_group_is_allowed() -> Result<()> {
    let env = Env("ALL ALL=() NOPASSWD: ALL")
        .group(USERNAME)
        .user(User(USERNAME).secondary_group(USERNAME))
        .build()?;

    for (user, group) in [("root", ROOT_GROUP), (USERNAME, USERNAME)] {
        Command::new("sudo")
            .args(["-g", group, "true"])
            .as_user(user)
            .output(&env)?
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
            .output(&env)?
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
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "I'm sorry root. I'm afraid I can't do that");
    }

    Ok(())
}

#[test]
fn when_specific_user_then_as_self_is_not_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=({USERNAME}) NOPASSWD: ALL")).build()?;

    let output = Command::new("sudo").args(["true"]).output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "I'm sorry root. I'm afraid I can't do that");
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
            .output(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        let diagnostic = if sudo_test::is_original_sudo() {
            format!(" is not allowed to execute '{BIN_TRUE}' as ")
        } else {
            format!("I'm sorry {user}. I'm afraid I can't do that")
        };
        assert_contains!(output.stderr(), diagnostic);
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
            .output(&env)?
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
            .output(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        let stderr = output.stderr();
        if sudo_test::is_original_sudo() {
            assert_snapshot!(stderr);
        } else {
            assert_contains!(
                stderr,
                format!("I'm sorry {user}. I'm afraid I can't do that")
            );
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
            .output(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        let stderr = output.stderr();
        if sudo_test::is_original_sudo() {
            assert_snapshot!(stderr);
        } else {
            assert_contains!(
                stderr,
                format!("I'm sorry {user}. I'm afraid I can't do that")
            );
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
            .output(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_both_user_and_group_are_specified_then_as_that_group_is_allowed() -> Result<()> {
    let env = Env(format!("ALL ALL=({USERNAME}:{GROUPNAME}) NOPASSWD: ALL"))
        .user(USERNAME)
        .group(GROUPNAME)
        .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-g", GROUPNAME, "true"])
            .as_user(user)
            .output(&env)?
            .assert_success()?;
    }

    Ok(())
}

// `man sudoers` says in the 'Runas_Spec' section
// "If no Runas_Spec is specified, the command may only be run as root and the group, if specified, must be one that root is a member of."
#[test]
fn when_no_run_as_spec_then_target_user_can_be_root() -> Result<()> {
    let env = Env("ALL ALL=NOPASSWD: ALL").user(USERNAME).build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn when_no_run_as_spec_then_target_user_cannot_be_a_regular_user() -> Result<()> {
    let env = Env("ALL ALL=NOPASSWD: ALL").user(USERNAME).build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "true"])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("user root is not allowed to execute '{BIN_TRUE}' as ferris")
    } else {
        "I'm sorry root. I'm afraid I can't do that".to_owned()
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn when_no_run_as_spec_then_an_arbitrary_target_group_may_not_be_specified() -> Result<()> {
    if sudo_test::is_original_sudo() {
        // TODO: original sudo should pass this test after 1.9.14b2
        return Ok(());
    }

    let env = Env("ALL ALL = NOPASSWD: ALL")
        .user(User(USERNAME))
        .group(GROUPNAME)
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", "root", "-g", GROUPNAME, "groups"])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("user {USERNAME} is not allowed to execute '{BIN_TRUE}' as root:{GROUPNAME}")
    } else {
        format!("I'm sorry {USERNAME}. I'm afraid I can't do that")
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn when_no_run_as_spec_then_a_group_that_root_is_in_may_be_specified() -> Result<()> {
    let env = Env("ALL ALL = NOPASSWD: ALL")
        .user(User(USERNAME))
        .group(GROUPNAME)
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", "root", "-g", ROOT_GROUP, "groups"])
        .as_user(USERNAME)
        .output(&env)?
        .stdout()?;

    let mut actual = output.split_ascii_whitespace().collect::<HashSet<_>>();

    assert!(actual.remove(ROOT_GROUP));
    assert!(actual.is_empty());

    Ok(())
}

#[test]
fn when_both_user_and_group_are_specified_then_as_that_user_with_that_group_is_allowed(
) -> Result<()> {
    let env = Env([&format!(
        "{USERNAME} ALL=(otheruser:{GROUPNAME}) NOPASSWD: ALL"
    )])
    .user(User(USERNAME))
    .user(User("otheruser"))
    .group(GROUPNAME)
    .build()?;

    Command::new("sudo")
        .args(["-u", "otheruser", "-g", GROUPNAME, "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
/// This test tracks [CVE-2009-0034](https://cve.mitre.org/cgi-bin/cvename.cgi?name=CVE-2009-0034)
/// which is explained in more detail [here](https://www.sudo.ws/security/advisories/group_vector/).
fn supplemental_group_matching() -> Result<()> {
    let other_user = "corro";

    let env = Env([&format!("{USERNAME} ALL=(%{GROUPNAME}) NOPASSWD: ALL")])
        .user(User(USERNAME).secondary_group(GROUPNAME))
        .user(User(other_user))
        .group(GROUPNAME)
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", other_user, "true"])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            format!("I'm sorry {USERNAME}. I'm afraid I can't do that")
        );
    }

    Ok(())
}

/// This test tracks [CVE-2019-14287](https://cve.mitre.org/cgi-bin/cvename.cgi?name=CVE-2019-14287)
/// which is explained in more detail [here](https://www.sudo.ws/security/advisories/minus_1_uid/).
#[test]
fn minus_1_uid() -> Result<()> {
    let host = "myhost";

    let env = Env(format!(
        "{USERNAME} {host} = (ALL, !root) NOPASSWD: /usr/bin/id"
    ))
    .user(User(USERNAME))
    .group(GROUPNAME)
    .hostname(host)
    .build()?;

    let needle = if sudo_test::is_original_sudo() {
        "unknown user"
    } else {
        "not found"
    };

    for uid in [-1i64, u32::MAX.into()] {
        let output = Command::new("sudo")
            .args(["-u", &format!("#{uid}"), "id", "-u"])
            .output(&env)?;

        assert!(!output.status().success());
        assert_contains!(output.stderr(), needle);
    }

    Ok(())
}

#[test]
fn null_byte_terminated_username() -> Result<()> {
    let env = Env("ferris ALL=(root\0:ALL) NOPASSWD: ALL")
        .user("ferris")
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user("ferris")
        .output(&env)?;

    assert!(!output.status().success());
    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "syntax error");
    } else {
        assert_contains!(output.stderr(), "expecting ')' but found '\0'");
    }

    Ok(())
}

#[test]
fn null_byte_terminated_groupname() -> Result<()> {
    let env = Env("ferris ALL=(ALL:root\0) NOPASSWD: ALL")
        .user("ferris")
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user("ferris")
        .output(&env)?;

    assert!(!output.status().success());
    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "syntax error");
    } else {
        assert_contains!(output.stderr(), "expecting ')' but found '\0'");
    }

    Ok(())
}
