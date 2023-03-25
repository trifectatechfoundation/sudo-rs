use std::collections::HashSet;

use pretty_assertions::assert_eq;
use sudo_test::{Command, Env, Group, User};

use crate::{Result, GROUPNAME, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

#[test]
fn changes_the_group_id() -> Result<()> {
    let expected_gid = 1234;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .group(Group(GROUPNAME).id(expected_gid))
        .build()?;

    for user in ["root", USERNAME] {
        let actual = Command::new("sudo")
            .args(["-g", GROUPNAME, "id", "-g"])
            .as_user(user)
            .exec(&env)?
            .stdout()?
            .parse::<u32>()?;

        assert_eq!(expected_gid, actual);
    }

    Ok(())
}

#[test]
fn adds_group_to_groups_output() -> Result<()> {
    let extra_group = "rustaceans";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(User(USERNAME).group("users"))
        .group(Group(extra_group))
        .build()?;

    for user in ["root", USERNAME] {
        let stdout = Command::new("groups").as_user(user).exec(&env)?.stdout()?;
        let groups_without_sudo = stdout.split_ascii_whitespace().collect::<HashSet<_>>();

        let stdout = Command::new("sudo")
            .args(["-g", GROUPNAME, "groups"])
            .as_user(user)
            .exec(&env)?
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
            .exec(&env)?
            .stdout()?
            .parse::<u32>()?;

        assert_eq!(expected_gid, actual);
    }

    Ok(())
}

#[test]
#[ignore]
fn can_use_unassigned_group_id() -> Result<()> {
    let expected_gid = 1234;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    for user in ["root", USERNAME] {
        let actual = Command::new("sudo")
            .arg("-g")
            .arg(format!("#{expected_gid}"))
            .args(["id", "-g"])
            .as_user(user)
            .exec(&env)?
            .stdout()?
            .parse::<u32>()?;

        assert_eq!(expected_gid, actual);
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
            .exec(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        if sudo_test::is_original_sudo() {
            assert_contains!(output.stderr(), "unknown group: ghosts");
        }
    }

    Ok(())
}
