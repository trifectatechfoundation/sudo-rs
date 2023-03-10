#![cfg(test)]

use std::collections::HashMap;

use pretty_assertions::assert_eq;
use sudo_test::{As, EnvBuilder};

type Error = Box<dyn std::error::Error>;
type Result<T> = core::result::Result<T, Error>;

macro_rules! assert_contains {
    ($haystack:expr, $needle:expr) => {
        let haystack = &$haystack;
        let needle = &$needle;

        assert!(
            haystack.contains(needle),
            "{haystack:?} did not contain {needle:?}"
        )
    };
}

fn parse_env_output(env_output: &str) -> Result<HashMap<&str, &str>> {
    let mut env = HashMap::new();
    for line in env_output.lines() {
        if let Some((key, value)) = line.split_once('=') {
            env.insert(key, value);
        }
    }

    Ok(env)
}

#[test]
fn cannot_sudo_with_empty_sudoers_file() -> Result<()> {
    let env = EnvBuilder::default().build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "root is not in the sudoers file");
    }

    Ok(())
}

#[test]
fn cannot_sudo_if_sudoers_file_is_world_writable() -> Result<()> {
    let env = EnvBuilder::default().sudoers_chmod("446").build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "/etc/sudoers is world writable");
    }

    Ok(())
}

// man sudoers > User Authentication:
// "A password is not required if the invoking user is root"
#[test]
fn can_sudo_as_root_without_providing_a_password_if_root_is_in_sudoers_file() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers("root    ALL=(ALL:ALL) ALL")
        .build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[test]
fn can_sudo_as_user_if_users_group_is_in_sudoers_file_and_correct_password_is_provided(
) -> Result<()> {
    let username = "ferris";
    let groupname = "rustaceans";
    let password = "strong-password";
    let env = EnvBuilder::default()
        .sudoers(&format!("%{groupname}    ALL=(ALL:ALL) ALL"))
        .user(username, &[groupname])
        .user_password(username, password)
        .build()?;

    let output = env.exec(
        &["sudo", "-S", "true"],
        As::User { name: username },
        Some(password),
    )?;
    assert!(output.status.success(), "{}", output.stderr);

    Ok(())
}

#[test]
fn cannot_sudo_as_user_if_users_group_is_in_sudoers_file_and_password_is_not_provided() -> Result<()>
{
    let username = "ferris";
    let groupname = "rustaceans";
    let password = "strong-password";
    let env = EnvBuilder::default()
        .sudoers(&format!("%{groupname}    ALL=(ALL:ALL) ALL"))
        .user(username, &[groupname])
        .user_password(username, password)
        .build()?;

    let output = env.exec(&["sudo", "-S", "true"], As::User { name: username }, None)?;
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "no password was provided");
    }

    Ok(())
}

#[test]
fn cannot_sudo_if_sudoers_has_invalid_syntax() -> Result<()> {
    let env = EnvBuilder::default().sudoers("invalid syntax").build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert!(!output.status.success());
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "syntax error");
    }

    Ok(())
}

// see 'environment' section in`man sudo`
// see 'command environment' section in`man sudoers`
#[test]
fn vars_set_by_sudo_in_env_reset_mode() -> Result<()> {
    // 'env_reset' is enabled by default
    let env = EnvBuilder::default()
        .sudoers("root    ALL=(ALL:ALL) ALL")
        .build()?;

    let stdout = env.stdout(&["env"], As::Root, None)?;
    let normal_env = parse_env_output(&stdout)?;

    let sudo_abs_path = env.stdout(&["which", "sudo"], As::Root, None)?;
    let env_abs_path = env.stdout(&["which", "env"], As::Root, None)?;

    // run sudo in an empty environment
    let stdout = env.stdout(
        &["env", "-i", &sudo_abs_path, &env_abs_path],
        As::Root,
        None,
    )?;
    let mut sudo_env = parse_env_output(&stdout)?;

    // # man sudo
    // "Set to the mail spool of the target user"
    assert_eq!(Some("/var/mail/root"), sudo_env.remove("MAIL"));

    // "Set to the home directory of the target user"
    assert_eq!(Some("/root"), sudo_env.remove("HOME"));

    // "Set to the login name of the target user"
    assert_eq!(Some("root"), sudo_env.remove("LOGNAME"));

    // "Set to the command run by sudo, including any args"
    assert_eq!(Some("/usr/bin/env"), sudo_env.remove("SUDO_COMMAND"));

    // "Set to the group-ID of the user who invoked sudo"
    assert_eq!(Some("0"), sudo_env.remove("SUDO_GID"));

    // "Set to the user-ID of the user who invoked sudo"
    assert_eq!(Some("0"), sudo_env.remove("SUDO_UID"));

    // "Set to the login name of the user who invoked sudo"
    assert_eq!(Some("root"), sudo_env.remove("SUDO_USER"));

    // "Set to the same value as LOGNAME"
    assert_eq!(Some("root"), sudo_env.remove("USER"));

    // # man sudoers
    // "The HOME, MAIL, SHELL, LOGNAME and USER environment variables are initialized based on the target user"
    assert_eq!(Some("/bin/bash"), sudo_env.remove("SHELL"));

    // "If the PATH and TERM variables are not preserved from the user's environment, they will be set to default values."
    let sudo_path = sudo_env.remove("PATH").expect("PATH not set");

    let normal_path = normal_env["PATH"];
    assert_ne!(normal_path, sudo_path);

    let default_path = "/usr/bin:/bin:/usr/sbin:/sbin";
    assert_eq!(default_path, sudo_path);

    let default_term = "unknown";
    assert_eq!(Some(default_term), sudo_env.remove("TERM"));

    let empty = HashMap::new();
    assert_eq!(empty, sudo_env);

    Ok(())
}
