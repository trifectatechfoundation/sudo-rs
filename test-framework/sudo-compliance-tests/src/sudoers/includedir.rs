use sudo_test::{Command, Directory, Env, TextFile};

use crate::{
    Result, SUDOERS_ALL_ALL_NOPASSWD, SUDOERS_USER_ALL_ALL, SUDOERS_USER_ALL_NOPASSWD, USERNAME,
};

#[test]
fn absolute_path() -> Result<()> {
    let env = Env("@includedir /etc/sudoers.d")
        .file("/etc/sudoers.d/a", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn relative_path() -> Result<()> {
    let env = Env("@includedir sudoers.d")
        .file("/etc/sudoers.d/a", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
#[ignore = "gh682"]
fn ignores_files_with_names_ending_in_tilde() -> Result<()> {
    let env = Env("@includedir /etc/sudoers.d")
        .file("/etc/sudoers.d/a~", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(output.stderr(), "root is not in the sudoers file");

    Ok(())
}

#[test]
#[ignore = "gh682"]
fn ignores_files_with_names_that_contain_a_dot() -> Result<()> {
    let env = Env("@includedir /etc/sudoers.d")
        .file("/etc/sudoers.d/a.", SUDOERS_ALL_ALL_NOPASSWD)
        .file("/etc/sudoers.d/.b", SUDOERS_ALL_ALL_NOPASSWD)
        .file("/etc/sudoers.d/c.d", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(output.stderr(), "root is not in the sudoers file");

    Ok(())
}

#[test]
#[ignore = "gh682"]
fn directory_does_not_exist_is_not_fatal() -> Result<()> {
    let env = Env("@includedir /etc/does-not-exist").build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(output.stderr(), "root is not in the sudoers file");

    Ok(())
}

#[test]
fn loads_files_in_lexical_order() -> Result<()> {
    let env = Env("@includedir /etc/sudoers.d")
        .file("/etc/sudoers.d/a", "ALL ALL=(ALL:ALL) ALL")
        .file("/etc/sudoers.d/b", "ALL ALL=(ALL:ALL) NOPASSWD: ALL")
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
#[ignore = "gh682"]
fn ignores_and_warns_about_file_with_bad_perms() -> Result<()> {
    let env = Env([SUDOERS_USER_ALL_NOPASSWD, "@includedir /etc/sudoers.d"])
        .file(
            "/etc/sudoers.d/a",
            // if this was NOT ignored, then the `sudo true` below would fail because no password
            // was provided
            TextFile(SUDOERS_USER_ALL_ALL).chmod("777"),
        )
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success());
    assert_contains!(output.stderr(), "/etc/sudoers.d/a is world writable");

    Ok(())
}

#[test]
#[ignore = "gh682"]
fn ignores_and_warns_about_file_with_bad_ownership() -> Result<()> {
    let env = Env([SUDOERS_USER_ALL_NOPASSWD, "@includedir /etc/sudoers.d"])
        .file(
            "/etc/sudoers.d/a",
            // if this was NOT ignored, then the `sudo true` below would fail because no password
            // was provided
            TextFile(SUDOERS_USER_ALL_ALL).chown(USERNAME),
        )
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success());
    assert_contains!(
        output.stderr(),
        "/etc/sudoers.d/a is owned by uid 1000, should be 0"
    );

    Ok(())
}

#[test]
#[ignore = "gh682"]
fn include_loop() -> Result<()> {
    let env = Env([SUDOERS_USER_ALL_NOPASSWD, "@includedir /etc/sudoers.d"])
        .file("/etc/sudoers.d/a", TextFile("@include /etc/sudoers.d/a"))
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success());
    assert_contains!(
        output.stderr(),
        "/etc/sudoers.d/a: too many levels of includes"
    );

    Ok(())
}

#[test]
#[ignore = "gh682"]
fn statements_prior_to_include_loop_are_evaluated() -> Result<()> {
    let env = Env([SUDOERS_USER_ALL_ALL, "@includedir /etc/sudoers.d"])
        .file(
            "/etc/sudoers.d/a",
            TextFile(format!(
                // if this first line was ignored the `sudo true` below would fail because a
                // password was not provided
                "{SUDOERS_USER_ALL_NOPASSWD}
@include /etc/sudoers.d/a"
            )),
        )
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success());
    assert_contains!(
        output.stderr(),
        "/etc/sudoers.d/a: too many levels of includes"
    );

    Ok(())
}

#[test]
fn whitespace_in_name_escaped() -> Result<()> {
    let env = Env(r#"@includedir /etc/sudo\ ers.d"#)
        .directory(r#"/etc/sudo ers.d"#)
        .file(r#"/etc/sudo ers.d/a"#, SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn whitespace_in_name_double_quotes() -> Result<()> {
    let env = Env(r#"@includedir "/etc/sudo ers.d" "#)
        .directory(r#"/etc/sudo ers.d"#)
        .file(r#"/etc/sudo ers.d/a"#, SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn backslash_in_name_escaped() -> Result<()> {
    let env = Env(r#"@includedir /etc/sudo\\ers.d"#)
        .directory(r#"/etc/sudo\ers.d"#)
        .file(r#"/etc/sudo\ers.d/a"#, SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn backslash_in_name_double_quotes() -> Result<()> {
    let env = Env(r#"@includedir "/etc/sudo\ers.d""#)
        .directory(r#"/etc/sudo\ers.d"#)
        .file(r#"/etc/sudo\ers.d/a"#, SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn old_pound_syntax() -> Result<()> {
    let env = Env("#includedir /etc/sudoers.d")
        .file("/etc/sudoers.d/a", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
#[ignore = "gh682"]
fn no_hostname_expansion() -> Result<()> {
    let hostname = "ship";
    let env = Env("@includedir /etc/sudoers.%h")
        .directory(format!("/etc/sudoers.{hostname}"))
        .file(
            format!("/etc/sudoers.{hostname}/a"),
            SUDOERS_ALL_ALL_NOPASSWD,
        )
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(output.stderr(), "root is not in the sudoers file");

    Ok(())
}

#[test]
#[ignore = "gh682"]
fn ignores_directory_with_bad_perms() -> Result<()> {
    let env = Env("@includedir /etc/sudoers2.d")
        .directory(Directory("/etc/sudoers2.d").chmod("777"))
        .file("/etc/sudoers2.d/a", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(output.stderr(), "sudo: /etc/sudoers2.d is world writable");
    assert_contains!(output.stderr(), "root is not in the sudoers file");

    Ok(())
}

#[test]
#[ignore = "gh682"]
fn ignores_directory_with_bad_ownership() -> Result<()> {
    let env = Env("@includedir /etc/sudoers2.d")
        .directory(Directory("/etc/sudoers2.d").chown(USERNAME))
        .file("/etc/sudoers2.d/a", SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(
        output.stderr(),
        "sudo: /etc/sudoers2.d is owned by uid 1000, should be 0"
    );
    assert_contains!(output.stderr(), "root is not in the sudoers file");

    Ok(())
}

#[test]
fn relative_path_parent_directory() -> Result<()> {
    let env = Env("@includedir ../sudoers.d")
        .directory("/sudoers.d")
        .file("/sudoers.d/a", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn relative_path_grandparent_directory() -> Result<()> {
    // base path is `/etc/` so grandparent does not exist
    let env = Env("@includedir ../../sudoers.d")
        .directory("/sudoers.d")
        .file("/sudoers.d/a", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn relative_path_dot_slash() -> Result<()> {
    // base path is `/etc/` so grandparent does not exist
    let env = Env("@includedir ./sudoers.d")
        .file("/etc/sudoers.d/a", SUDOERS_ALL_ALL_NOPASSWD)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}
