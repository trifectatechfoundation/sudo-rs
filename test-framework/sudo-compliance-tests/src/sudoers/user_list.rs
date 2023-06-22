//! Test the first component of the user specification: `<user_list> ALL=(ALL:ALL) ALL`

use pretty_assertions::assert_eq;
use sudo_test::{Command, Env, User};

use crate::{Result, PAMD_SUDO_PAM_PERMIT, SUDOERS_NO_LECTURE, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/sudoers/user_list",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn no_match() -> Result<()> {
    let env = Env("").build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;
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
fn all() -> Result<()> {
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn user_name() -> Result<()> {
    let env = Env("root ALL=(ALL:ALL) NOPASSWD: ALL").build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn user_id() -> Result<()> {
    let env = Env("#0 ALL=(ALL:ALL) NOPASSWD: ALL").build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn group_name() -> Result<()> {
    let env = Env("%root ALL=(ALL:ALL) NOPASSWD: ALL").build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn group_id() -> Result<()> {
    let env = Env("%#0 ALL=(ALL:ALL) NOPASSWD: ALL").build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn many_different() -> Result<()> {
    let env = Env(format!("root, {USERNAME} ALL=(ALL:ALL) NOPASSWD: ALL"))
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn many_repeated() -> Result<()> {
    let env = Env("root, root ALL=(ALL:ALL) NOPASSWD: ALL").build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn double_negative_is_positive() -> Result<()> {
    let env = Env("!!root ALL=(ALL:ALL) NOPASSWD: ALL")
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn negation_excludes_group_members() -> Result<()> {
    let env = Env(["%users, !ghost ALL=(ALL:ALL) ALL", SUDOERS_NO_LECTURE])
        // use PAM to avoid `ghost` getting a password prompt
        .file("/etc/pam.d/sudo", PAMD_SUDO_PAM_PERMIT)
        // the primary group of all new users is `users`
        .user("ferris")
        .user("ghost")
        .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user("ferris")
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user("ghost")
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "authentication failed: I'm sorry ghost. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn negation_is_order_sensitive() -> Result<()> {
    // negated items at the start of a specifier list  are meaningless
    let env = Env("!ghost, %users ALL=(ALL:ALL) NOPASSWD: ALL")
        // the primary group of all new users is `users`
        .user("ferris")
        .user("ghost")
        .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user("ferris")
        .output(&env)?
        .assert_success()?;

    Command::new("sudo")
        .arg("true")
        .as_user("ghost")
        .output(&env)?
        .assert_success()
}

#[test]
fn user_alias_works() -> Result<()> {
    let env = Env([
        "User_Alias ADMINS = %users, !ghost",
        "ADMINS ALL=(ALL:ALL) ALL",
        SUDOERS_NO_LECTURE,
    ])
    // use PAM to avoid password prompts
    .file("/etc/pam.d/sudo", PAMD_SUDO_PAM_PERMIT)
    // the primary group of all new users is `users`
    .user("ferris")
    .user("ghost")
    .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user("ferris")
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user("ghost")
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "authentication failed: I'm sorry ghost. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn user_alias_can_contain_underscore_and_digits() -> Result<()> {
    let env = Env([
        "User_Alias UNDER_SCORE123 = ALL",
        "UNDER_SCORE123 ALL = (ALL:ALL) NOPASSWD: /usr/bin/true",
    ])
    .user(USERNAME)
    .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn user_alias_cannot_start_with_underscore() -> Result<()> {
    let env = Env([
        "User_Alias _FOO = ALL",
        "ALL ALL = (ALL:ALL) NOPASSWD: /usr/bin/true",
        "_FOO ALL = (ALL:ALL) PASSWD: ALL",
    ])
    .user(USERNAME)
    .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn negated_user_alias_works() -> Result<()> {
    let env = Env("
User_Alias ADMINS = %users, !ghost
!ADMINS ALL=(ALL:ALL) ALL")
    // use PAM to avoid password prompts
    .file("/etc/pam.d/sudo", PAMD_SUDO_PAM_PERMIT)
    // the primary group of all new users is `users`
    .user("ferris")
    .user("ghost")
    .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user("ghost")
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user("ferris")
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "ferris is not in the sudoers file"
    } else {
        "authentication failed: I'm sorry ferris. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn negated_subgroup() -> Result<()> {
    let env = Env(["%users, !%rustaceans ALL=(ALL:ALL) ALL", SUDOERS_NO_LECTURE])
        // use PAM to avoid password prompts
        .file("/etc/pam.d/sudo", PAMD_SUDO_PAM_PERMIT)
        // the primary group of all new users is `users`
        .group("rustaceans")
        .user(User("ferris").secondary_group("rustaceans"))
        .user("ghost")
        .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user("ghost")
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user("ferris")
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_snapshot!(output.stderr());
    }

    Ok(())
}

#[test]
fn negated_supergroup() -> Result<()> {
    let env = Env(["%rustaceans, !%users ALL=(ALL:ALL) ALL", SUDOERS_NO_LECTURE])
        // use PAM to avoid password prompts
        .file("/etc/pam.d/sudo", PAMD_SUDO_PAM_PERMIT)
        // the primary group of all new users is `users`
        .group("rustaceans")
        .user(User("ferris").secondary_group("rustaceans"))
        .user("ghost")
        .build()?;

    for user in ["ferris", "ghost"] {
        let output = Command::new("sudo")
            .arg("true")
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
                format!("authentication failed: I'm sorry {user}. I'm afraid I can't do that")
            );
        }
    }

    Ok(())
}
