use sudo_test::{Command, Env, TextFile, User};

use crate::{helpers, Result, PASSWORD, USERNAME};

#[test]
fn vars_home_and_shell_are_preserved_for_root() -> Result<()> {
    let env = Env("").build()?;

    let home = "my-home";
    let shell = "/usr/bin/env";
    let stdout = Command::new("env")
        .arg(format!("HOME={home}"))
        .arg(format!("SHELL={shell}"))
        .args(["su", "--preserve-environment"])
        .output(&env)?
        .stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;
    dbg!(&su_env);

    assert_eq!(Some(home), su_env.get("HOME").copied());
    assert_eq!(Some(shell), su_env.get("SHELL").copied());

    Ok(())
}

#[test]
fn vars_home_shell_user_and_logname_are_preserved_for_reg_user() -> Result<()> {
    let env = Env("").user(USERNAME).build()?;

    let home = "my-home";
    let shell = "/usr/bin/env";
    let user = "my-user";
    let logname = "my-logname";
    let stdout = Command::new("env")
        .arg(format!("HOME={home}"))
        .arg(format!("SHELL={shell}"))
        .arg(format!("USER={user}"))
        .arg(format!("LOGNAME={logname}"))
        .args(["su", "--preserve-environment"])
        .output(&env)?
        .stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;
    dbg!(&su_env);

    assert_eq!(Some(home), su_env.get("HOME").copied());
    assert_eq!(Some(shell), su_env.get("SHELL").copied());
    assert_eq!(Some(user), su_env.get("USER").copied());
    assert_eq!(Some(logname), su_env.get("LOGNAME").copied());

    Ok(())
}

#[test]
fn uses_shell_env_var_when_flag_preserve_environment_is_present() -> Result<()> {
    let env = Env("").build()?;

    let cases = [("/usr/bin/true", None), ("/usr/bin/false", Some(1))];

    for (shell, code) in cases {
        let output = Command::new("env")
            .arg(format!("SHELL={shell}"))
            .args(["su", "--preserve-environment"])
            .output(&env)?;

        assert_eq!(code.is_none(), output.status().success());
        if code.is_some() {
            assert_eq!(code, output.status().code());
        }
    }

    Ok(())
}

#[test]
#[ignore = "wontfix"]
fn may_be_specified_more_than_once_without_change_in_semantics() -> Result<()> {
    let env = Env("").build()?;

    let home = "my-home";
    let shell = "/usr/bin/env";
    let stdout = Command::new("env")
        .arg(format!("HOME={home}"))
        .arg(format!("SHELL={shell}"))
        .args(["su", "--preserve-environment", "-p"])
        .output(&env)?
        .stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;
    dbg!(&su_env);

    assert_eq!(Some(home), su_env.get("HOME").copied());
    assert_eq!(Some(shell), su_env.get("SHELL").copied());

    Ok(())
}

#[test]
fn shell_env_var_is_ignored_when_target_user_has_a_restricted_shell_and_invoking_user_is_not_root(
) -> Result<()> {
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

    let output = Command::new("env")
        .args(["SHELL=/usr/bin/false", "su", "-p", target_user])
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
