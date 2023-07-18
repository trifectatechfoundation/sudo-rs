use std::collections::HashSet;

use pretty_assertions::assert_eq;
use sudo_test::{Command, Env, Group, User};

use crate::{Result, GROUPNAME, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/flag_group",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn changes_the_real_and_effective_group_id() -> Result<()> {
    let expected_gid = 1234;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .group(Group(GROUPNAME).id(expected_gid))
        .build()?;

    for user in ["root", USERNAME] {
        let effective_gid = Command::new("sudo")
            .args(["-g", GROUPNAME, "id", "-g"])
            .as_user(user)
            .output(&env)?
            .stdout()?
            .parse::<u32>()?;

        let real_gid = Command::new("sudo")
            .args(["-g", GROUPNAME, "id", "-r", "-g"])
            .as_user(user)
            .output(&env)?
            .stdout()?
            .parse::<u32>()?;

        assert_eq!(expected_gid, effective_gid);
        assert_eq!(expected_gid, real_gid);
    }

    Ok(())
}

#[test]
fn adds_group_to_groups_output() -> Result<()> {
    let extra_group = "rustaceans";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(User(USERNAME).secondary_group("secondary-group"))
        .group(Group(extra_group))
        .group("secondary-group")
        .build()?;

    for user in ["root", USERNAME] {
        let stdout = Command::new("groups")
            .as_user(user)
            .output(&env)?
            .stdout()?;
        let groups_without_sudo = stdout.split_ascii_whitespace().collect::<HashSet<_>>();

        let stdout = Command::new("sudo")
            .args(["-g", GROUPNAME, "groups"])
            .as_user(user)
            .output(&env)?
            .stdout()?;
        let mut groups_with_sudo = stdout.split_ascii_whitespace().collect::<HashSet<_>>();

        assert!(groups_with_sudo.remove(extra_group));

        assert_eq!(groups_with_sudo, groups_without_sudo);
    }

    Ok(())
}

#[test]
fn group_can_be_specified_by_id() -> Result<()> {
    let expected_gid = 1234;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .group(Group(GROUPNAME).id(expected_gid))
        .build()?;

    for user in ["root", USERNAME] {
        let actual = Command::new("sudo")
            .arg("-g")
            .arg(format!("#{expected_gid}"))
            .args(["id", "-g"])
            .as_user(user)
            .output(&env)?
            .stdout()?
            .parse::<u32>()?;

        assert_eq!(expected_gid, actual);
    }

    Ok(())
}

#[test]
fn unassigned_group_id_is_rejected() -> Result<()> {
    let expected_gid = 1234;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    for user in ["root", USERNAME] {
        let output = Command::new("sudo")
            .arg("-g")
            .arg(format!("#{expected_gid}"))
            .arg("true")
            .as_user(user)
            .output(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        let stderr = output.stderr();
        if sudo_test::is_original_sudo() {
            assert_snapshot!(stderr);
        } else {
            assert_contains!(stderr, "group '#1234' not found");
        }
    }

    Ok(())
}

#[test]
fn group_does_not_exist() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    for user in ["root", USERNAME] {
        let output = Command::new("sudo")
            .args(["-g", "ghosts", "true"])
            .as_user(user)
            .output(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        let diagnostic = if sudo_test::is_original_sudo() {
            "unknown group ghosts"
        } else {
            "group 'ghosts' not found"
        };
        assert_contains!(output.stderr(), diagnostic);
    }

    Ok(())
}

#[test]
fn group_does_not_add_groups_without_authorization() -> Result<()> {
    let env = Env("ALL ALL=(ALL:rustaceans) NOPASSWD: ALL")
        .user(USERNAME)
        .group("rustaceans")
        .group("elite")
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "-g", "elite", "true"])
        .as_user(USERNAME)
        .output(&env)?;

    assert!(!output.status().success());

    let diagnostic = if sudo_test::is_original_sudo() {
        "a password is required"
    } else {
        "authentication failed: I'm sorry ferris. I'm afraid I can't do that"
    };

    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

// " If no `-u` option is specified, the command will be run as the invoking user."
#[test]
fn if_no_flag_user_then_target_user_is_the_invoking_user() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .group(Group(GROUPNAME))
        .build()?;

    for invoking_user in ["root", USERNAME] {
        let target_user = Command::new("sudo")
            .args(["-g", GROUPNAME, "whoami"])
            .as_user(invoking_user)
            .output(&env)?
            .stdout()?;

        assert_eq!(invoking_user, target_user);
    }

    Ok(())
}
