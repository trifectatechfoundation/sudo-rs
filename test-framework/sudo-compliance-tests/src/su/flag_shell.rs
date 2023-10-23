use sudo_test::{Command, Env, TextFile, User};

use crate::{Result, PASSWORD, USERNAME};

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
fn specified_shell_does_not_exist() -> Result<()> {
    let env = Env("").build()?;

    let command_path = "/does/not/exist";
    let output = Command::new("su").args(["-s", command_path]).output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(127), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("su: failed to execute {command_path}: No such file or directory")
    } else {
        format!("su: '{command_path}': command not found")
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn specified_shell_could_not_be_executed() -> Result<()> {
    let shell_path = "/tmp/my-shell";
    let env = Env("").file(shell_path, "").build()?;

    let output = Command::new("su").args(["-s", shell_path]).output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(126), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("su: failed to execute {shell_path}: Permission denied")
    } else {
        format!("su: '{shell_path}': invalid command")
    };

    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn ignores_shell_env_var_when_flag_preserve_environment_is_absent() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("env")
        .arg("SHELL=/usr/bin/false")
        .args(["su", "-c", "echo $SHELL"])
        .output(&env)?
        .stdout()?;

    assert_eq!("/bin/bash", stdout);

    Ok(())
}

#[test]
fn ignored_when_target_user_has_a_restricted_shell_and_invoking_user_is_not_root() -> Result<()> {
    let invoking_user = USERNAME;
    let target_user = "ghost";
    let message = "this is a restricted shell";
    let restricted_shell_path = "/tmp/restricted-shell";
    let restricted_shell = format!(
        "#!/bin/sh
echo {message}"
    );
    let env = Env("")
        .file(
            restricted_shell_path,
            TextFile(restricted_shell).chmod("777"),
        )
        .user(invoking_user)
        .user(
            User(target_user)
                .shell(restricted_shell_path)
                .password(PASSWORD),
        )
        .build()?;

    // restricted shell = "a shell not in /etc/shells"
    let etc_shells = Command::new("cat")
        .arg("/etc/shells")
        .output(&env)?
        .stdout()?;
    assert_not_contains!(etc_shells, restricted_shell_path);

    let output = Command::new("su")
        .args(["-s", "/usr/bin/false", target_user])
        .stdin(PASSWORD)
        .as_user(invoking_user)
        .output(&env)?;

    assert!(output.status().success(), "{}", output.stderr());
    assert_contains!(
        output.stderr(),
        format!("su: using restricted shell {restricted_shell_path}")
    );

    assert_eq!(message, output.stdout()?);

    Ok(())
}

#[test]
fn when_specified_more_than_once_last_value_is_used() -> Result<()> {
    let shell_path = "/root/my-shell";
    let shell = "#!/bin/sh
echo $0";
    let env = Env("")
        .file(shell_path, TextFile(shell).chmod("100"))
        .build()?;

    let actual = Command::new("su")
        .args(["-s", "/usr/bin/env"])
        .args(["-s", "/usr/bin/false"])
        .args(["-s", shell_path])
        .output(&env)?
        .stdout()?;

    assert_eq!(shell_path, actual);

    Ok(())
}

#[test]
fn commented_out_shell_in_etc_shells_is_ignored() -> Result<()> {
    let invoking_user = USERNAME;
    let target_user = "ghost";
    let message = "this is a restricted shell";
    let restricted_shell_path = "/tmp/restricted-shell";
    let restricted_shell = format!(
        "#!/bin/sh
echo {message}"
    );
    let env = Env("")
        .file(
            restricted_shell_path,
            TextFile(restricted_shell).chmod("777"),
        )
        .file(
            "/etc/shells",
            format!(
                "# {restricted_shell_path}
/usr/bin/sh
/usr/bin/bash"
            ),
        )
        .user(invoking_user)
        .user(
            User(target_user)
                .shell(restricted_shell_path)
                .password(PASSWORD),
        )
        .build()?;

    let output = Command::new("su")
        .args(["-s", "/usr/bin/false", target_user])
        .stdin(PASSWORD)
        .as_user(invoking_user)
        .output(&env)?;

    assert!(output.status().success(), "{}", output.stderr());
    assert_contains!(
        output.stderr(),
        format!("su: using restricted shell {restricted_shell_path}")
    );

    assert_eq!(message, output.stdout()?);

    Ok(())
}

#[test]
fn when_no_etc_shells_file_uses_a_default_list() -> Result<()> {
    let default_list = ["/bin/sh"];
    let not_in_list = [
        "/bin/bash",
        "/usr/bin/bash",
        "/usr/bin/sh",
        "/bin/dash",
        "/usr/bin/dash",
    ];
    let invoking_user = USERNAME;
    let target_user = "ghost";

    for shell in not_in_list {
        eprintln!("out: {shell}");

        let env = Env("")
            .user(invoking_user)
            .user(User(target_user).shell(shell).password(PASSWORD))
            .build()?;

        Command::new("rm")
            .arg("/etc/shells")
            .output(&env)?
            .assert_success()?;

        let output = Command::new("su")
            .args(["-s", "/usr/bin/false", target_user])
            .stdin(PASSWORD)
            .as_user(invoking_user)
            .output(&env)?;

        assert!(output.status().success(), "{}", output.stderr());
        assert_contains!(
            output.stderr(),
            format!("su: using restricted shell {shell}")
        );
    }

    for shell in default_list {
        eprintln!("in: {shell}");

        let env = Env("")
            .user(invoking_user)
            .user(User(target_user).shell(shell).password(PASSWORD))
            .build()?;

        Command::new("rm")
            .arg("/etc/shells")
            .output(&env)?
            .assert_success()?;

        let output = Command::new("su")
            .args(["-s", "/usr/bin/true", "-c", "false", target_user])
            .stdin(PASSWORD)
            .as_user(invoking_user)
            .output(&env)?;

        assert!(output.status().success(), "{}", output.stderr());
        assert_not_contains!(output.stderr(), format!("su: using restricted shell"));
    }

    Ok(())
}

#[test]
fn shell_canonical_path_is_not_used_when_determining_if_shell_is_restricted_or_not() -> Result<()> {
    let invoking_user = USERNAME;
    let target_user = "ghost";
    let shell = "/tmp/bash-symlink";

    let env = Env("")
        .file("/etc/shells", "/usr/bin/bash")
        .user(invoking_user)
        .user(User(target_user).shell(shell).password(PASSWORD))
        .build()?;

    Command::new("ln")
        .args(["-s", "/usr/bin/bash", shell])
        .output(&env)?
        .assert_success()?;

    Command::new("rm")
        .arg("/etc/shells")
        .output(&env)?
        .assert_success()?;

    let output = Command::new("su")
        .args(["-s", "/usr/bin/false", target_user])
        .stdin(PASSWORD)
        .as_user(invoking_user)
        .output(&env)?;

    assert!(output.status().success(), "{}", output.stderr());
    assert_contains!(
        output.stderr(),
        format!("su: using restricted shell {shell}")
    );

    Ok(())
}

#[test]
fn shell_is_resolved_with_empty_path_env_var() -> Result<()> {
    let env = Env("").build()?;

    let command_path = "true";
    let output = Command::new("su").args(["-s", command_path]).output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(127), output.status().code());

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("su: failed to execute {command_path}: No such file or directory")
    } else {
        format!("su: '{command_path}': command not found")
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}

#[test]
fn argument_may_be_a_relative_path() -> Result<()> {
    let shell_filename = "my-shell";
    let shell_dir = "/root";
    let shell = "#!/bin/sh
echo $0";
    let env = Env("")
        .file(
            format!("{shell_dir}/{shell_filename}"),
            TextFile(shell).chmod("100"),
        )
        .build()?;

    let actual = Command::new("sh")
        .arg("-c")
        .arg(format!("cd {shell_dir}; su -s ./{shell_filename}"))
        .output(&env)?
        .stdout()?;

    assert_eq!(format!("./{shell_filename}"), actual);

    Ok(())
}

#[test]
fn positional_arguments_are_passed_to_shell() -> Result<()> {
    let shell_path = "/root/my-shell";
    let args = ["a", "b"];
    let shell = "#!/bin/sh
echo ${@}";
    let env = Env("")
        .file(shell_path, TextFile(shell).chmod("100"))
        .build()?;

    let actual = Command::new("su")
        .args(["-s", shell_path, "root"])
        .args(args)
        .output(&env)?
        .stdout()?;

    assert_eq!(args.join(" "), actual);

    Ok(())
}
