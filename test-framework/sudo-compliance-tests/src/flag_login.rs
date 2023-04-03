use sudo_test::{Command, Env, TextFile, User};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

#[test]
fn works_even_if_home_directory_does_not_exist() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "-i", "true"])
        .exec(&env)?;

    assert!(output.status().success());

    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "sudo: unable to change directory to /home/ferris: No such file or directory"
        );
    }

    Ok(())
}

#[test]
#[ignore]
fn sets_home_directory_as_working_directory() -> Result<()> {
    let expected = format!("/home/{USERNAME}");
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(User(USERNAME).create_home_directory())
        .build()?;

    let actual = Command::new("sh")
        .arg("-c")
        .arg(format!("cd /; sudo -u {USERNAME} -i pwd"))
        .exec(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
#[ignore]
fn uses_shell_in_passwd_database() -> Result<()> {
    let my_shell = "#!/bin/sh
echo $0";
    let shell_path = "/tmp/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chown(USERNAME).chmod("500"))
        .user(User(USERNAME).shell(shell_path))
        .build()?;

    let actual = Command::new("sudo")
        .args(["-u", USERNAME, "-i"])
        .exec(&env)?
        .stdout()?;
    let expected = shell_path;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
#[ignore]
fn argument_is_invoke_with_dash_c_flag() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(User(USERNAME).shell(shell_path))
        .file(shell_path, TextFile(my_shell).chown(USERNAME).chmod("500"))
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "-i", "argument"])
        .exec(&env)?
        .stdout()?;

    assert_eq!("-c argument", output);

    Ok(())
}

#[test]
#[ignore]
fn arguments_are_concatenated_with_whitespace() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(User(USERNAME).shell(shell_path))
        .file(shell_path, TextFile(my_shell).chown(USERNAME).chmod("500"))
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "-i", "a", "b"])
        .exec(&env)?
        .stdout()?;

    assert_eq!("-c a b", output);

    Ok(())
}

#[test]
#[ignore]
fn arguments_are_escaped_with_backslashes() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(User(USERNAME).shell(shell_path))
        .file(shell_path, TextFile(my_shell).chown(USERNAME).chmod("500"))
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "-i", "'", "\"", "a b"])
        .exec(&env)?
        .stdout()?;

    assert_eq!(r#"-c \' \" a\ b"#, output);

    Ok(())
}

#[test]
#[ignore]
fn alphanumerics_underscores_hyphens_and_dollar_signs_are_not_escaped() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(User(USERNAME).shell(shell_path))
        .file(shell_path, TextFile(my_shell).chown(USERNAME).chmod("500"))
        .build()?;

    let output = Command::new("sudo")
        .args([
            "-u", USERNAME, "-i", "a", "1", "_", "-", "$", "$VAR", "${VAR}",
        ])
        .exec(&env)?
        .stdout()?;

    assert_eq!(r#"-c a 1 _ - $ $VAR $\{VAR\}"#, output);

    Ok(())
}

#[test]
fn shell_does_not_exist() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(User(USERNAME).shell(shell_path))
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "-i"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "sudo: /tmp/my-shell: command not found");
    }

    Ok(())
}

#[test]
fn insufficient_permissions_to_execute_shell() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile("#!/bin/sh").chmod("100"))
        .user(User(USERNAME).shell(shell_path))
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "-i"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(
            output.stderr(),
            "sudo: unable to execute /tmp/my-shell: Permission denied"
        );
    }

    Ok(())
}

#[test]
#[ignore]
fn shell_with_open_permissions_is_accepted() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile("#!/bin/sh").chmod("777"))
        .user(User(USERNAME).shell(shell_path))
        .build()?;

    Command::new("sudo")
        .args(["-u", USERNAME, "-i"])
        .exec(&env)?
        .assert_success()
}
