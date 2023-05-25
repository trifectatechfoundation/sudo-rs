use sudo_test::{Command, Env, TextFile, User};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "snapshots/flag_login",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn if_home_directory_does_not_exist_executes_program_without_changing_the_working_directory(
) -> Result<()> {
    let initial_working_directories = ["/", "/root"];

    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;
    for expected in initial_working_directories {
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("cd {expected}; sudo -u {USERNAME} -i pwd"))
            .exec(&env)?;

        assert!(output.status().success());

        let stderr = output.stderr();
        if sudo_test::is_original_sudo() {
            assert_snapshot!(stderr);
        } else {
            assert_contains!(stderr, "unable to change directory");
        }

        let actual = output.stdout()?;
        assert_eq!(actual, expected);
    }

    Ok(())
}

#[test]
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
fn uses_target_users_shell_in_passwd_database() -> Result<()> {
    let my_shell = "#!/bin/sh
echo $0";
    let shell_path = "/tmp/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chown(USERNAME).chmod("500"))
        .user(User(USERNAME).shell(shell_path))
        .build()?;

    // the invoking user's (root's) shell (`bash` or `sh`) is clearly not the target user's shell so
    // we don't assert that they are different

    let actual = Command::new("sudo")
        .args(["-u", USERNAME, "-i"])
        .exec(&env)?
        .stdout()?;
    let expected = shell_path;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
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
fn arguments_are_properly_distinguished() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let my_shell = "#!/bin/sh
for arg in \"$@\"; do echo -n \"{$arg}\"; done";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(User(USERNAME).shell(shell_path))
        .file(shell_path, TextFile(my_shell).chown(USERNAME).chmod("500"))
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "-i", "a b", "c d"])
        .exec(&env)?
        .stdout()?;

    assert_eq!("{-c}{a\\ b c\\ d}", output);

    Ok(())
}

#[test]
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
fn shell_is_invoked_as_a_login_shell() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(User(USERNAME).shell("/bin/bash"))
        .build()?;

    let expected = "-bash";
    let actual = Command::new("sudo")
        .args(["-u", "ferris", "-i", "echo", "$0"])
        .exec(&env)?
        .stdout()?;

    // man bash says "A login shell is one whose first character of argument zero is a -"
    assert_eq!(expected, actual);

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

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "IO error: No such file or directory");
    }

    Ok(())
}

#[test]
fn insufficient_permissions_to_execute_shell() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile("#!/bin/sh").chmod("100"))
        .user(User(USERNAME).shell(shell_path).create_home_directory())
        .build()?;

    let output = Command::new("sudo")
        .args(["-u", USERNAME, "-i"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "IO error: Permission denied");
    }

    Ok(())
}

#[test]
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
