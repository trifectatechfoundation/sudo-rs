use std::collections::HashMap;

use sudo_test::{Command, Env, TextFile};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "snapshots/flag_shell",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn if_shell_env_var_is_not_set_then_uses_the_invoking_users_shell_in_passwd_database() -> Result<()>
{
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    let getent_passwd = Command::new("getent")
        .arg("passwd")
        .output(&env)?
        .stdout()?;
    let user_to_shell = parse_getent_passwd_output(&getent_passwd)?;
    let target_users_shell = user_to_shell["root"];
    let invoking_users_shell = user_to_shell["ferris"];

    // XXX a bit brittle. it would be better to set ferris' shell when creating the user
    assert_ne!(target_users_shell, invoking_users_shell);

    let output = Command::new("env")
        .args(["-u", "SHELL", "sudo", "-s", "echo", "$0"])
        .as_user(USERNAME)
        .output(&env)?
        .stdout()?;

    assert_eq!(invoking_users_shell, output);

    Ok(())
}

#[test]
fn if_shell_env_var_is_set_then_uses_it() -> Result<()> {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $0";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build()?;

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s"])
        .output(&env)?
        .stdout()?;

    assert_eq!(shell_path, output);

    Ok(())
}

#[test]
fn argument_is_invoked_with_dash_c_flag() -> Result<()> {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build()?;

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s", "argument"])
        .output(&env)?
        .stdout()?;

    assert_eq!("-c argument", output);

    Ok(())
}

#[test]
fn arguments_are_concatenated_with_whitespace() -> Result<()> {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build()?;

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s", "a", "b"])
        .output(&env)?
        .stdout()?;

    assert_eq!("-c a b", output);

    Ok(())
}

#[test]
fn arguments_are_properly_distinguished() -> Result<()> {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
for arg in \"$@\"; do echo -n \"{$arg}\"; done";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build()?;

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s", "a b", "c d"])
        .output(&env)?
        .stdout()?;

    assert_eq!("{-c}{a\\ b c\\ d}", output);

    Ok(())
}

#[test]
fn arguments_are_escaped_with_backslashes() -> Result<()> {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build()?;

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s", "'", "\"", "a b"])
        .output(&env)?
        .stdout()?;

    assert_eq!(r#"-c \' \" a\ b"#, output);

    Ok(())
}

#[test]
fn alphanumerics_underscores_hyphens_and_dollar_signs_are_not_escaped() -> Result<()> {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build()?;

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s", "a", "1", "_", "-", "$", "$VAR", "${VAR}"])
        .output(&env)?
        .stdout()?;

    assert_eq!(r#"-c a 1 _ - $ $VAR $\{VAR\}"#, output);

    Ok(())
}

#[test]
fn shell_is_not_invoked_as_a_login_shell() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let actual = Command::new("env")
        .args(["SHELL=/bin/bash", "sudo", "-s", "echo", "$0"])
        .output(&env)?
        .stdout()?;

    // man bash says "A login shell is one whose first character of argument zero is a -"
    assert_ne!("-bash", actual);
    assert_eq!("/bin/bash", actual);

    Ok(())
}

#[test]
fn shell_does_not_exist() -> Result<()> {
    let shell_path = "/root/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s"])
        .output(&env)?;

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
fn shell_is_not_executable() -> Result<()> {
    let shell_path = "/root/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile("#!/bin/sh").chmod("000"))
        .build()?;

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s"])
        .output(&env)?;

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
        .build()?;

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s"])
        .output(&env)?;

    assert!(output.status().success());

    Ok(())
}

type UserToShell<'a> = HashMap<&'a str, &'a str>;

fn parse_getent_passwd_output(passwd: &str) -> Result<UserToShell> {
    const ERROR: &str = "malformed `getent passwd` output";
    let mut map = HashMap::new();
    for line in passwd.lines() {
        let Some((user, _)) = line.split_once(':')  else { return Err(ERROR.into())};
        let Some((_, shell)) = line.rsplit_once(':')  else { return Err(ERROR.into())};

        map.insert(user, shell);
    }
    Ok(map)
}
