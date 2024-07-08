use pretty_assertions::assert_eq;
use sudo_test::{Command, Env, User};

use crate::{
    Result, GROUPNAME, SUDOERS_ALL_ALL_NOPASSWD, SUDOERS_ROOT_ALL_NOPASSWD,
    SUDOERS_USER_ALL_NOPASSWD, USERNAME,
};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/flag_user",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn root_can_become_another_user_by_name() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).user(USERNAME).build()?;

    // NOTE `id` without flags prints the *real* user/group id if it's different from the
    // *effective* user/group id so here we are checking that both the real *and* effective UID has
    // changed to match the target user's
    let expected = Command::new("id")
        .as_user(USERNAME)
        .output(&env)?
        .stdout()?;
    let actual = Command::new("sudo")
        .args(["-u", USERNAME, "id"])
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[ignore = "gh680"]
#[test]
fn uppercase_u_flag_fails() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).user(USERNAME).build()?;

    let output = Command::new("sudo")
        .args(["-U", USERNAME, "id"])
        .output(&env)?;
    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    assert_contains!(
        stderr,
        "sudo: the -U option may only be used with the -l option"
    );

    Ok(())
}

#[test]
fn root_can_become_another_user_by_uid() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).user(USERNAME).build()?;

    let uid = Command::new("id")
        .arg("-u")
        .as_user(USERNAME)
        .output(&env)?
        .stdout()?
        .parse::<u32>()?;
    let expected = Command::new("id")
        .as_user(USERNAME)
        .output(&env)?
        .stdout()?;
    let actual = Command::new("sudo")
        .arg("-u")
        .arg(format!("#{uid}"))
        .arg("id")
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn user_can_become_another_user() -> Result<()> {
    let invoking_user = USERNAME;
    let another_user = "another_user";
    let env = Env(SUDOERS_USER_ALL_NOPASSWD)
        .user(invoking_user)
        .user(another_user)
        .build()?;

    let expected = Command::new("id")
        .as_user(another_user)
        .output(&env)?
        .stdout()?;
    let actual = Command::new("sudo")
        .args(["-u", another_user, "id"])
        .as_user(USERNAME)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

// regression test for trifectatechfoundation/sudo-rs#81
#[test]
fn invoking_user_groups_are_lost_when_becoming_another_user() -> Result<()> {
    let invoking_user = USERNAME;
    let another_user = "another_user";
    let env = Env(SUDOERS_USER_ALL_NOPASSWD)
        .group(GROUPNAME)
        .user(User(invoking_user).secondary_group(GROUPNAME))
        .user(another_user)
        .build()?;

    let expected = Command::new("id")
        .as_user(another_user)
        .output(&env)?
        .stdout()?;
    let actual = Command::new("sudo")
        .args(["-u", another_user, "id"])
        .as_user(invoking_user)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn unassigned_user_id_is_rejected() -> Result<()> {
    let expected_uid = 1234;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    for user in ["root", USERNAME] {
        let output = Command::new("sudo")
            .arg("-u")
            .arg(format!("#{expected_uid}"))
            .arg("true")
            .as_user(user)
            .output(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        let stderr = output.stderr();
        if sudo_test::is_original_sudo() {
            assert_snapshot!(stderr);
        } else {
            assert_contains!(stderr, "user '#1234' not found");
        }
    }

    Ok(())
}

#[test]
fn user_does_not_exist() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let output = Command::new("sudo")
        .args(["-u", "ghost", "true"])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "unknown user ghost"
    } else {
        "user 'ghost' not found"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}
