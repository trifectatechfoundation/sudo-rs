use std::collections::HashMap;

use sudo_test::{Command, Env, User};

use crate::{helpers, Result, ENV_PATH, USERNAME};

#[test]
fn it_works() -> Result<()> {
    let env = Env("").build()?;

    let argss = [["-c", "echo $0", "-l"], ["-c", "echo $0", "-"]];

    for args in argss {
        let actual = Command::new("su").args(args).output(&env)?.stdout()?;

        // argv[0] is prefixed with '-' to invoke the shell as a login shell
        assert_eq!("-bash", actual);
    }

    Ok(())
}

#[test]
fn vars_set_by_su_when_target_is_not_root() -> Result<()> {
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let stdout = Command::new("env")
        .args(["-i", "/usr/bin/su", "-l", USERNAME])
        .output(&env)?
        .stdout()?;
    let mut su_env = helpers::parse_env_output(&stdout)?;

    dbg!(&su_env);

    assert_eq!(Some(ENV_PATH), su_env.remove("SHELL"));
    assert_eq!(
        Some(format!("/home/{USERNAME}")).as_deref(),
        su_env.remove("HOME")
    );
    assert_eq!(Some(USERNAME), su_env.remove("USER"));
    assert_eq!(Some(USERNAME), su_env.remove("LOGNAME"));
    assert_eq!(
        Some(format!("/var/mail/{USERNAME}")).as_deref(),
        su_env.remove("MAIL")
    );
    // NOTE unlikely the no-`--login` case, PATH gets set
    // this could come from `/etc/login.defs` or `/etc/profile`
    assert_eq!(
        Some("/usr/local/bin:/usr/bin:/bin:/usr/local/games:/usr/games"),
        su_env.remove("PATH")
    );

    assert_eq!(HashMap::new(), su_env);

    Ok(())
}

#[test]
fn vars_set_by_su_when_target_is_root() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("env")
        .args(["-i", "/usr/bin/su", "-s", ENV_PATH, "-l"])
        .output(&env)?
        .stdout()?;
    let mut su_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(ENV_PATH), su_env.remove("SHELL"));
    assert_eq!(Some("/root"), su_env.remove("HOME"));
    assert_eq!(Some("root"), su_env.remove("USER"));
    assert_eq!(Some("root"), su_env.remove("LOGNAME"));
    assert_eq!(Some("/var/mail/root"), su_env.remove("MAIL"));
    // NOTE unlikely the no-`--login` case, PATH gets set
    // this could come from `/etc/login.defs` or `/etc/profile`
    assert_eq!(
        Some("/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"),
        su_env.remove("PATH")
    );

    assert_eq!(HashMap::new(), su_env);

    Ok(())
}

#[test]
fn clears_vars_in_invoking_user_environment() -> Result<()> {
    let varname = "SHOULD_BE_REMOVED";
    let varval = "42";
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let stdout = Command::new("env")
        .arg(format!("{varname}={varval}"))
        .args(["su", "-l", USERNAME])
        .output(&env)?
        .stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(None, su_env.get(varname).copied());

    Ok(())
}

#[test]
fn has_precedence_over_flag_preserve_environment() -> Result<()> {
    let varname = "SHOULD_BE_REMOVED";
    let varval = "42";
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let stdout = Command::new("env")
        .arg(format!("{varname}={varval}"))
        .args(["su", "-p", "-l", USERNAME])
        .output(&env)?
        .stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(None, su_env.get(varname).copied());

    Ok(())
}

#[test]
#[ignore = "gh532"]
fn term_var_in_invoking_users_env_is_preserved() -> Result<()> {
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let term = "my-term";
    let stdout = Command::new("env")
        .arg(format!("TERM={term}"))
        .args(["su", "-p", "-l", USERNAME])
        .output(&env)?
        .stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(term), su_env.get("TERM").copied());

    Ok(())
}

#[test]
fn may_be_specified_more_than_once_without_change_in_semantics() -> Result<()> {
    let env = Env("").build()?;

    let argss = [
        &["-c", "echo $0", "-l", "-l"],
        &["-c", "echo $0", "-l", "-"],
    ];

    for args in argss {
        dbg!(args);

        let actual = Command::new("su").args(args).output(&env)?.stdout()?;

        // argv[0] is prefixed with '-' to invoke the shell as a login shell
        assert_eq!("-bash", actual);
    }

    Ok(())
}
