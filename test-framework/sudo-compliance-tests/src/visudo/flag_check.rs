use sudo_test::{Command, Env, TextFile};

use crate::{visudo::ETC_SUDOERS, Result, USERNAME};

const DEFAULT_CHMOD: &str = "440";

#[test]
#[ignore = "gh657"]
fn no_syntax_errors_and_ok_ownership_and_perms() -> Result<()> {
    let env = Env(TextFile("").chmod(DEFAULT_CHMOD)).build()?;

    let output = Command::new("visudo").arg("-c").output(&env)?;

    assert!(output.status().success());
    assert!(output.stderr().is_empty());
    assert_eq!("/etc/sudoers: parsed OK", output.stdout()?);

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn bad_perms() -> Result<()> {
    let env = Env(TextFile("").chmod("444")).build()?;

    let output = Command::new("visudo").arg("-c").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_eq!(
        "/etc/sudoers: bad permissions, should be mode 0440",
        output.stderr()
    );

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn bad_ownership() -> Result<()> {
    let env = Env(TextFile("").chown(USERNAME).chmod(DEFAULT_CHMOD))
        .user(USERNAME)
        .build()?;

    let output = Command::new("visudo").arg("-c").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_eq!(
        "/etc/sudoers: wrong owner (uid, gid) should be (0, 0)",
        output.stderr()
    );

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn bad_syntax() -> Result<()> {
    let env = Env(TextFile("this is fine").chmod(DEFAULT_CHMOD)).build()?;

    let output = Command::new("visudo").arg("-c").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(output.stderr(), "syntax error");

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn file_does_not_exist() -> Result<()> {
    let env = Env("").build()?;

    Command::new("rm")
        .args(["-f", ETC_SUDOERS])
        .output(&env)?
        .assert_success()?;

    let output = Command::new("visudo").arg("-c").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_eq!(
        "visudo: unable to open /etc/sudoers: No such file or directory",
        output.stderr()
    );

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn flag_quiet_ok() -> Result<()> {
    let env = Env(TextFile("").chmod(DEFAULT_CHMOD)).build()?;

    let output = Command::new("visudo").args(["-c", "-q"]).output(&env)?;

    assert!(output.status().success());
    assert!(output.stderr().is_empty());
    assert!(output.stdout()?.is_empty());

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn flag_quiet_bad_perms() -> Result<()> {
    let env = Env(TextFile("").chmod("444")).build()?;

    let output = Command::new("visudo").args(["-c", "-q"]).output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert!(output.stderr().is_empty());

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn flag_quiet_bad_ownership() -> Result<()> {
    let env = Env(TextFile("").chmod(DEFAULT_CHMOD).chown(USERNAME))
        .user(USERNAME)
        .build()?;

    let output = Command::new("visudo").args(["-c", "-q"]).output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert!(output.stderr().is_empty());

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn flag_quiet_bad_syntax() -> Result<()> {
    let env = Env(TextFile("this is fine").chmod(DEFAULT_CHMOD)).build()?;

    let output = Command::new("visudo").args(["-c", "-q"]).output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert!(output.stderr().is_empty());

    Ok(())
}
