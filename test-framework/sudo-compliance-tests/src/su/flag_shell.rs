use sudo_test::{Command, Env, TextFile, User};

use crate::{Result, USERNAME};

#[test]
fn it_works() -> Result<()> {
    let shell_path = "/root/my-shell";
    let shell = "#!/bin/sh
echo $0";
    let env = Env("")
        .file(shell_path, TextFile(shell).chmod("100"))
        .build()?;

    let actual = Command::new("su")
        .args(["-s", shell_path])
        .output(&env)?
        .stdout()?;

    assert_eq!(shell_path, actual);

    Ok(())
}

#[test]
fn default_shell_is_the_one_in_target_users_passwd_db_entry() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let shell = "#!/bin/sh
echo $0";
    let env = Env("")
        .user(User(USERNAME).shell(shell_path))
        .file(shell_path, TextFile(shell).chmod("777"))
        .build()?;

    let actual = Command::new("su").arg(USERNAME).output(&env)?.stdout()?;

    assert_eq!(shell_path, actual);

    Ok(())
}

#[test]
#[ignore = "gh502"]
fn specified_shell_does_not_exist() -> Result<()> {
    let env = Env("").build()?;

    let output = Command::new("su")
        .args(["-s", "/does/not/exist"])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(127), output.status().code());

    let diagnostic = "su: failed to execute /does/not/exist: No such file or directory";
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
#[ignore = "gh503"]
fn specified_shell_could_not_be_executed() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let env = Env("").file(shell_path, "").build()?;

    let output = Command::new("su").args(["-s", shell_path]).output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(126), output.status().code());

    let diagnostic = format!("su: failed to execute {shell_path}: Permission denied");
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
#[ignore = "gh507"]
fn ignores_shell_env_var_when_flag_preserve_environment_is_absent() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("env")
        .arg("SHELL=/usr/bin/false")
        .args(["su", "-c", "echo $0"])
        .output(&env)?
        .stdout()?;

    assert_eq!("bash", stdout);

    Ok(())
}
