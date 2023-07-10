use sudo_test::{Command, Env, TextFile};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

#[test]
fn relative_path() -> Result<()> {
    let env = Env("@include sudoers2")
        .file("/etc/sudoers2", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn absolute_path() -> Result<()> {
    let env = Env("@include /root/sudoers")
        .file("/root/sudoers", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn file_does_not_exist() -> Result<()> {
    let env = Env("@include sudoers2").build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(
        output.stderr(),
        "sudo: unable to open /etc/sudoers2: No such file or directory"
    );
    Ok(())
}

#[test]
fn whitespace_in_name_backslash() -> Result<()> {
    let env = Env(r#"@include sudo\ ers"#)
        .file("/etc/sudo ers", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn whitespace_in_name_double_quotes() -> Result<()> {
    let env = Env(r#"@include "sudo ers" "#)
        .file("/etc/sudo ers", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn old_pound_syntax() -> Result<()> {
    let env = Env("#include sudoers2")
        .file("/etc/sudoers2", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn backslash_in_name() -> Result<()> {
    let env = Env(r#"@include sudo\\ers"#)
        .file(r#"/etc/sudo\ers"#, SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn backslash_in_name_double_quotes() -> Result<()> {
    let env = Env(r#"@include "sudo\ers" "#)
        .file(r#"/etc/sudo\ers"#, SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn include_loop_error_messages() -> Result<()> {
    let env = Env("@include sudoers2")
        .file(r#"/etc/sudoers2"#, "@include sudoers")
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(
        output.stderr(),
        "sudo: /etc/sudoers2: too many levels of includes"
    );

    Ok(())
}

#[test]
fn include_loop_not_fatal() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "@include sudoers2"])
        .file(r#"/etc/sudoers2"#, "@include sudoers")
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(output.status().success());
    assert_contains!(
        output.stderr(),
        "sudo: /etc/sudoers2: too many levels of includes"
    );

    Ok(())
}

#[test]
fn permissions_check() -> Result<()> {
    let env = Env("@include sudoers2")
        .file(
            r#"/etc/sudoers2"#,
            TextFile(SUDOERS_ALL_ALL_NOPASSWD).chmod("777"),
        )
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(output.stderr(), "sudo: /etc/sudoers2 is world writable");

    Ok(())
}

#[test]
fn permissions_check_not_fatal() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "@include sudoers2"])
        .file(r#"/etc/sudoers2"#, TextFile("").chmod("777"))
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(output.status().success());
    assert_contains!(output.stderr(), "sudo: /etc/sudoers2 is world writable");

    Ok(())
}

#[test]
fn ownership_check() -> Result<()> {
    let env = Env("@include sudoers2")
        .file(
            r#"/etc/sudoers2"#,
            TextFile(SUDOERS_ALL_ALL_NOPASSWD).chown(USERNAME),
        )
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(
        output.stderr(),
        "sudo: /etc/sudoers2 is owned by uid 1000, should be 0"
    );

    Ok(())
}

#[test]
fn ownership_check_not_fatal() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "@include sudoers2"])
        .file(r#"/etc/sudoers2"#, TextFile("").chown(USERNAME))
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(output.status().success());
    assert_contains!(
        output.stderr(),
        "sudo: /etc/sudoers2 is owned by uid 1000, should be 0"
    );

    Ok(())
}

#[test]
fn hostname_expansion() -> Result<()> {
    let hostname = "ship";
    let env = Env("@include sudoers.%h")
        .file(format!("/etc/sudoers.{hostname}"), SUDOERS_ALL_ALL_NOPASSWD)
        .hostname(hostname)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}
