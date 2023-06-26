use sudo_test::{Command, Env, User};

use crate::{helpers, Result, ENV_PATH, USERNAME};

#[test]
fn it_works() -> Result<()> {
    let varname1 = "SHOULD_BE_PRESERVED";
    let varval1 = "42";
    let varname2 = "SHOULD_BE_REMOVED";
    let varval2 = "24";
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let stdout = Command::new("env")
        .arg(format!("{varname1}={varval1}"))
        .arg(format!("{varname2}={varval2}"))
        .args(["su", "-w", varname1, "-l", USERNAME])
        .output(&env)?
        .stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(varval1), su_env.get(varname1).copied());
    // not in `-w` list
    assert_eq!(None, su_env.get(varname2).copied());

    Ok(())
}

#[test]
fn list_syntax() -> Result<()> {
    let varname1 = "FOO";
    let varval1 = "42";
    let varname2 = "BAR";
    let varval2 = "24";
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let stdout = Command::new("env")
        .arg(format!("{varname1}={varval1}"))
        .arg(format!("{varname2}={varval2}"))
        .args([
            "su",
            "-w",
            &format!("{varname1},{varname2}"),
            "-l",
            USERNAME,
        ])
        .output(&env)?
        .stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(varval1), su_env.get(varname1).copied());
    assert_eq!(Some(varval2), su_env.get(varname2).copied());

    Ok(())
}

#[test]
fn cannot_preserve_home_shell_user_logname_or_path() -> Result<()> {
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let name_values = [
        ("HOME", "my-home"),
        ("SHELL", "my-shell"),
        ("USER", "my-user"),
        ("LOGNAME", "my-logname"),
        ("PATH", "my-path"),
    ];

    let mut command = Command::new("env");
    for (name, value) in name_values {
        command.arg(format!("{name}={value}"));
    }
    command.args(["/usr/bin/su", "-w"]);
    command.arg(
        name_values
            .iter()
            .map(|(name, _value)| *name)
            .collect::<Vec<_>>()
            .join(","),
    );
    let stdout = command.args(["-l", USERNAME]).output(&env)?.stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;

    for (name, value) in name_values {
        assert_ne!(Some(value), su_env.get(name).copied());
    }

    Ok(())
}

#[test]
fn list_syntax_odd_names() -> Result<()> {
    let varname1 = " A.";
    let varval1 = "42";
    let varname2 = "1 ";
    let varval2 = "24";
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let stdout = Command::new("env")
        .arg(format!("{varname1}={varval1}"))
        .arg(format!("{varname2}={varval2}"))
        .args([
            "su",
            "-w",
            &format!("{varname1},{varname2}"),
            "-l",
            USERNAME,
        ])
        .output(&env)?
        .stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(varval1), su_env.get(varname1).copied());
    assert_eq!(Some(varval2), su_env.get(varname2).copied());

    Ok(())
}

#[test]
fn when_specified_more_than_once_lists_are_merged() -> Result<()> {
    let varname1 = "FOO";
    let varval1 = "42";
    let varname2 = "BAR";
    let varval2 = "24";
    let varname3 = "BAZ";
    let varval3 = "33";
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let stdout = Command::new("env")
        .arg(format!("{varname1}={varval1}"))
        .arg(format!("{varname2}={varval2}"))
        .arg(format!("{varname3}={varval3}"))
        .args([
            "su",
            "-w",
            &format!("{varname1},{varname2}"),
            "-w",
            varname3,
            "-l",
            USERNAME,
        ])
        .output(&env)?
        .stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(varval1), su_env.get(varname1).copied());
    assert_eq!(Some(varval2), su_env.get(varname2).copied());
    assert_eq!(Some(varval3), su_env.get(varname3).copied());

    Ok(())
}
