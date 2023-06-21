use std::collections::HashMap;

use sudo_test::{Command, Env, User};

use crate::{helpers, Result, ENV_PATH, USERNAME};

#[test]
fn it_works() -> Result<()> {
    let env = Env("").build()?;

    let argss = [
        ["-l", "-c", "echo $0"],
        ["-", "-c", "echo $0"],
        ["-c", "echo $0", "-"],
    ];

    for args in argss {
        let actual = Command::new("su").args(args).output(&env)?.stdout()?;

        // argv[0] is prefixed with '-' to invoke the shell as a login shell
        assert_eq!("-bash", actual);
    }

    Ok(())
}

#[test]
#[ignore = "gh505"]
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
#[ignore = "gh504"]
fn vars_set_by_su_when_target_is_root() -> Result<()> {
    let env = Env("").build()?;

    let stdout = Command::new("env")
        .args(["-i", "/usr/bin/su", "-s", ENV_PATH, "-l"])
        .output(&env)?
        .stdout()?;
    let mut su_env = helpers::parse_env_output(&stdout)?;

    dbg!(&su_env);

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
