use std::collections::HashMap;

use sudo_test::{Command, Env, TextFile};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD};

#[test]
#[ignore]
fn if_shell_env_var_is_not_set_then_uses_the_shell_in_passwd_database() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let getent_passwd = Command::new("getent").arg("passwd").exec(&env)?.stdout()?;
    let user_to_shell = parse_getent_passwd_output(&getent_passwd)?;
    let root_shell = user_to_shell["root"];

    let output = Command::new("env")
        .args(["-u", "SHELL", "sudo", "-s", "echo", "$0"])
        .exec(&env)?
        .stdout()?;

    assert_eq!(root_shell, output);

    Ok(())
}

#[test]
#[ignore]
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
        .exec(&env)?
        .stdout()?;

    assert_eq!(shell_path, output);

    Ok(())
}

#[test]
#[ignore]
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
        .exec(&env)?
        .stdout()?;

    assert_eq!("-c argument", output);

    Ok(())
}

#[test]
#[ignore]
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
        .exec(&env)?
        .stdout()?;

    assert_eq!("-c a b", output);

    Ok(())
}

#[test]
#[ignore]
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
        .exec(&env)?
        .stdout()?;

    assert_eq!(r#"-c \' \" a\ b"#, output);

    Ok(())
}

#[test]
#[ignore]
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
        .exec(&env)?
        .stdout()?;

    assert_eq!(r#"-c a 1 _ - $ $VAR $\{VAR\}"#, output);

    Ok(())
}

#[test]
fn shell_does_not_exist() -> Result<()> {
    let shell_path = "/root/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "sudo: /root/my-shell: command not found");
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
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "sudo: /root/my-shell: command not found");
    }

    Ok(())
}

#[test]
#[ignore]
fn shell_with_open_permissions_is_accepted() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile("#!/bin/sh").chmod("777"))
        .build()?;

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s"])
        .exec(&env)?;

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
