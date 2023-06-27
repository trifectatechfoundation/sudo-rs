use std::collections::HashMap;

use sudo_test::{Command, Env, User};

use crate::{helpers, Result, ENV_PATH, USERNAME};

#[test]
fn vars_set_by_su_when_target_is_root() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("env")
        .args(["-i", "/usr/bin/su", "-s", ENV_PATH])
        .output(&env)?
        .stdout()?;
    let mut su_env = helpers::parse_env_output(&stdout)?;

    dbg!(&su_env);

    // NOTE `man su` says that su sets HOME and SHELL but here we observe that MAIL is also set
    assert_eq!(Some(ENV_PATH), su_env.remove("SHELL"));
    assert_eq!(Some("/root"), su_env.remove("HOME"));
    assert_eq!(Some("/var/mail/root"), su_env.remove("MAIL"));

    // remove profiling environment var
    let _ = su_env.remove("__LLVM_PROFILE_RT_INIT_ONCE");

    assert_eq!(HashMap::new(), su_env);

    Ok(())
}

#[test]
fn vars_set_by_su_when_target_is_not_root() -> Result<()> {
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let stdout = Command::new("env")
        .args(["-i", "/usr/bin/su", USERNAME])
        .output(&env)?
        .stdout()?;
    let mut su_env = helpers::parse_env_output(&stdout)?;

    dbg!(&su_env);

    // NOTE `man su` says that aside from HOME and SHELL, USER and LOGNAME are set when the target
    // user is not root
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

    // remove profiling environment var
    let _ = su_env.remove("__LLVM_PROFILE_RT_INIT_ONCE");

    assert_eq!(HashMap::new(), su_env);

    Ok(())
}

#[test]
fn vars_set_by_su_override_existing_ones() -> Result<()> {
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let stdout = Command::new("env")
        .arg("-i")
        // FIXME workaround for gh506. change to `SHELL=my-shell`
        .arg("SHELL=/usr/bin/env")
        .arg("HOME=my-home")
        .arg("USER=my-user")
        .arg("LOGNAME=my-logname")
        .arg("MAIL=my-mail")
        .args(["/usr/bin/su", USERNAME])
        .output(&env)?
        .stdout()?;
    let mut su_env = helpers::parse_env_output(&stdout)?;

    dbg!(&su_env);

    // NOTE `man su` says that aside from HOME and SHELL, USER and LOGNAME are set
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

    // remove profiling environment var
    let _ = su_env.remove("__LLVM_PROFILE_RT_INIT_ONCE");

    assert_eq!(HashMap::new(), su_env);

    Ok(())
}

#[test]
fn vars_in_invoking_users_env_are_preserved() -> Result<()> {
    let varname = "SHOULD_BE_PRESERVED";
    let varval = "42";
    let env = Env("").user(User(USERNAME).shell(ENV_PATH)).build()?;

    let stdout = Command::new("env")
        .arg("-i")
        .arg(format!("{varname}={varval}"))
        .args(["/usr/bin/su", USERNAME])
        .output(&env)?
        .stdout()?;
    let su_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(varval), su_env.get(varname).copied());

    Ok(())
}
