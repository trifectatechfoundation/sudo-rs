use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};
use sudo_test::{Command, Env, TextFile, BIN_PWD};

#[test]
fn cwd_not_set_cannot_change_dir() -> Result<()> {
    let env = Env(TextFile(SUDOERS_ALL_ALL_NOPASSWD)).build()?;

    let output = Command::new("sudo")
        .args(["--chdir", "/root", "pwd"])
        .output(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert!(!output.status().success());
    let diagnostic = if sudo_test::is_original_sudo() {
        format!("you are not permitted to use the -D option with {BIN_PWD}")
    } else {
        format!("you are not allowed to use '--chdir /root' with '{BIN_PWD}'")
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn cwd_set_to_glob_change_dir() -> Result<()> {
    let env = Env(TextFile("ALL ALL=(ALL:ALL) CWD=* NOPASSWD: ALL")).build()?;
    let output = Command::new("sh")
        .args(["-c", "cd /; sudo --chdir /root pwd"])
        .output(&env)?;
    assert_eq!(Some(0), output.status().code());
    assert!(output.status().success());
    assert_contains!(output.stdout()?, "/root");

    Ok(())
}

#[test]
fn cwd_fails_for_non_existent_dirs() -> Result<()> {
    let env = Env(TextFile("ALL ALL=(ALL:ALL) CWD=* NOPASSWD: ALL")).build()?;
    let output = Command::new("sudo")
        .args([
            "--chdir",
            "/path/to/nowhere",
            "sh",
            "-c",
            "echo >&2 'avocado'",
        ])
        .output(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert!(!output.status().success());
    let stderr = output.stderr();
    assert_contains!(
        stderr,
        "unable to change directory to /path/to/nowhere: No such file or directory"
    );
    assert_not_contains!(stderr, "avocado");

    Ok(())
}

#[test]
fn cwd_with_login_fails_for_non_existent_dirs() -> Result<()> {
    let env = Env(TextFile("ALL ALL=(ALL:ALL) CWD=* NOPASSWD: ALL"))
        .user(USERNAME)
        .build()?;
    let output = Command::new("sudo")
        .args([
            "-u",
            USERNAME,
            "-i",
            "--chdir",
            "/path/to/nowhere",
            "sh",
            "-c",
            "echo >&2 'avocado'",
        ])
        .output(&env)?;
    assert_eq!(Some(1), output.status().code());
    assert!(!output.status().success());
    let stderr = output.stderr();
    assert_contains!(
        stderr,
        "unable to change directory to /path/to/nowhere: No such file or directory"
    );
    assert_not_contains!(stderr, "avocado");

    Ok(())
}

#[test]
fn cwd_set_to_non_glob_value_then_cannot_use_chdir_flag() -> Result<()> {
    let env = Env(TextFile("ALL ALL=(ALL:ALL) CWD=/root NOPASSWD: ALL")).build()?;
    let output = Command::new("sh")
        .args(["-c", "cd /; sudo --chdir /tmp pwd"])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("you are not permitted to use the -D option with {BIN_PWD}")
    } else {
        format!("you are not allowed to use '--chdir /tmp' with '{BIN_PWD}'")
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn cwd_set_to_non_glob_value_then_cannot_use_that_path_with_chdir_flag() -> Result<()> {
    let path = "/root";
    let env = Env(format!("ALL ALL=(ALL:ALL) CWD={path} NOPASSWD: ALL")).build()?;
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("cd /; sudo --chdir {path} pwd"))
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("you are not permitted to use the -D option with {BIN_PWD}")
    } else {
        format!("you are not allowed to use '--chdir {path}' with '{BIN_PWD}'")
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn any_chdir_value_is_not_accepted_if_it_matches_pwd_cwd_unset() -> Result<()> {
    let path = "/root";
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: ALL").build()?;

    if sudo_test::is_original_sudo() {
        let stdout = Command::new("sudo")
            .arg("--version")
            .output(&env)?
            .stdout()?;
        let version = stdout
            .lines()
            .next()
            .unwrap()
            .strip_prefix("Sudo version ")
            .unwrap();
        if version < "1.9.14" {
            // Older sudo had a special case where --chdir is accepted if it matches the cwd even if
            // it would otherwise be denied.
            // FIXME remove once bookworm is oldstable
            return Ok(());
        }
    }

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("cd {path}; sudo --chdir {path} pwd"))
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("you are not permitted to use the -D option with {BIN_PWD}")
    } else {
        format!("you are not allowed to use '--chdir {path}' with '{BIN_PWD}'")
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn any_chdir_value_is_not_accepted_if_it_matches_pwd_cwd_set() -> Result<()> {
    let cwd_path = "/root";
    let another_path = "/tmp";
    let env = Env(format!("ALL ALL=(ALL:ALL) CWD={cwd_path} NOPASSWD: ALL")).build()?;

    if sudo_test::is_original_sudo() {
        let stdout = Command::new("sudo")
            .arg("--version")
            .output(&env)?
            .stdout()?;
        let version = stdout
            .lines()
            .next()
            .unwrap()
            .strip_prefix("Sudo version ")
            .unwrap();
        if version < "1.9.14" {
            // Older sudo had a special case where --chdir is accepted if it matches the cwd even if
            // it would otherwise be denied.
            // FIXME remove once bookworm is oldstable
            return Ok(());
        }
    }

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "cd {another_path}; sudo --chdir {another_path} pwd"
        ))
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("you are not permitted to use the -D option with {BIN_PWD}")
    } else {
        format!("you are not allowed to use '--chdir {another_path}' with '{BIN_PWD}'")
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn target_user_has_insufficient_perms() -> Result<()> {
    let path = "/root";
    let env = Env("ALL ALL=(ALL:ALL) CWD=* NOPASSWD: ALL")
        .user(USERNAME)
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("cd /; sudo -u {USERNAME} --chdir {path} pwd"))
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: unable to change directory to /root: Permission denied"
    } else {
        "Permission denied"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn flag_login_is_respected() -> Result<()> {
    let expected = "-sh";
    let env = Env("ALL ALL=(ALL:ALL) CWD=* ALL").build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg("sudo --login --chdir /tmp echo '$0'")
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, output);

    Ok(())
}
