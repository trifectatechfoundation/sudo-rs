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
    let env = Env("@include /etc/sudoers2").build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: unable to open /etc/sudoers2: No such file or directory"
    } else {
        "sudo-rs: cannot open sudoers file '/etc/sudoers2'"
    };
    assert_contains!(output.stderr(), diagnostic);
    Ok(())
}

#[test]
fn whitespace_in_name_backslash() -> Result<()> {
    let env = Env(r#"@include /etc/sudo\ ers"#)
        .file("/etc/sudo ers", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn whitespace_in_name_double_quotes() -> Result<()> {
    let env = Env(r#"@include "/etc/sudo ers" "#)
        .file("/etc/sudo ers", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn old_pound_syntax() -> Result<()> {
    let env = Env("#include /etc/sudoers2")
        .file("/etc/sudoers2", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn backslash_in_name() -> Result<()> {
    let env = Env(r#"@include /etc/sudo\\ers"#)
        .file(r#"/etc/sudo\ers"#, SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn backslash_in_name_double_quotes() -> Result<()> {
    let env = Env(r#"@include "/etc/sudo\ers" "#)
        .file(r#"/etc/sudo\ers"#, SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn double_quote_in_name_double_quotes() -> Result<()> {
    let env = Env(r#"@include "/etc/sudo\"ers" "#)
        .file(r#"/etc/sudo"ers"#, SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn include_loop_error_messages() -> Result<()> {
    let env = Env("@include /etc/sudoers2")
        .file(r#"/etc/sudoers2"#, "@include /etc/sudoers")
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: /etc/sudoers2: too many levels of includes"
    } else {
        "sudo-rs: include file limit reached opening '/etc/sudoers2'"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn include_loop_not_fatal() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "@include /etc/sudoers2"])
        .file(r#"/etc/sudoers2"#, "@include /etc/sudoers")
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(output.status().success());
    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: /etc/sudoers2: too many levels of includes"
    } else {
        "sudo-rs: include file limit reached opening '/etc/sudoers2'"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn permissions_check() -> Result<()> {
    let env = Env("@include /etc/sudoers2")
        .file(
            r#"/etc/sudoers2"#,
            TextFile(SUDOERS_ALL_ALL_NOPASSWD).chmod("777"),
        )
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: /etc/sudoers2 is world writable"
    } else {
        "sudo-rs: /etc/sudoers2 cannot be world-writable"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn permissions_check_not_fatal() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "@include sudoers2"])
        .file(r#"/etc/sudoers2"#, TextFile("").chmod("777"))
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(output.status().success());
    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: /etc/sudoers2 is world writable"
    } else {
        "sudo-rs: /etc/sudoers2 cannot be world-writable"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn ownership_check() -> Result<()> {
    let env = Env("@include /etc/sudoers2")
        .file(
            r#"/etc/sudoers2"#,
            TextFile(SUDOERS_ALL_ALL_NOPASSWD).chown(USERNAME),
        )
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: /etc/sudoers2 is owned by uid 1000, should be 0"
    } else {
        "sudo-rs: /etc/sudoers2 must be owned by root"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn ownership_check_not_fatal() -> Result<()> {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "@include /etc/sudoers2"])
        .file(r#"/etc/sudoers2"#, TextFile("").chown(USERNAME))
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(output.status().success());
    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: /etc/sudoers2 is owned by uid 1000, should be 0"
    } else {
        "sudo-rs: /etc/sudoers2 must be owned by root"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
#[ignore = "gh676"]
fn hostname_expansion() -> Result<()> {
    let hostname = "ship";
    let env = Env("@include /etc/sudoers.%h")
        .file(format!("/etc/sudoers.{hostname}"), SUDOERS_ALL_ALL_NOPASSWD)
        .hostname(hostname)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn relative_path_parent_directory() -> Result<()> {
    let env = Env("@include ../sudoers2")
        .file("/sudoers2", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn relative_path_grandparent_directory() -> Result<()> {
    // base path is `/etc/sudoers` so grandparent does not exist
    let env = Env("@include ../../sudoers2").build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: unable to open /etc/../../sudoers2: No such file or directory"
    } else {
        "sudo-rs: cannot open sudoers file '/etc/../../sudoers2'"
    };
    assert_contains!(output.stderr(), diagnostic);
    Ok(())
}

#[test]
fn relative_path_dot_slash() -> Result<()> {
    let env = Env("@include ./sudoers2")
        .file("/etc/sudoers2", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}
