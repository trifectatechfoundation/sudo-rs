// `pam_env` module integration
// see 'Command execution' section in `man sudoers`

use pretty_assertions::assert_eq;
use sudo_test::{Command, Env};

use crate::{helpers, EnvList, Result, SUDOERS_ALL_ALL_NOPASSWD};

const ETC_ENVIRONMENT_PATH: &str = "/etc/environment";
const PAM_D_SUDO_PATH: &str = "/etc/pam.d/sudo";
const SECURITY_PAM_ENV_PATH: &str = "/etc/security/pam_env.conf";

const STOCK_PAM_D_SUDO: &str = "#%PAM-1.0\n@include common-auth\n@include common-account\n@include common-session-noninteractive";
const STOCK_SECURITY_PAM_ENV: &str = "";

const PAM_D_SUDO_READENV: &str = "session       required   pam_env.so readenv=1";

fn remove_comments_and_whitespace(contents: &str) -> String {
    contents
        .trim()
        .lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty() && (line.starts_with("#%PAM") || !line.starts_with('#'))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn stock_pam_d_sudo() -> Result<()> {
    let env = Env("").build()?;

    if sudo_test::is_original_sudo() {
        let pam_d_sudo = Command::new("cat")
            .arg(PAM_D_SUDO_PATH)
            .exec(&env)?
            .stdout()?;

        assert_eq!(
            STOCK_PAM_D_SUDO,
            remove_comments_and_whitespace(&pam_d_sudo),
            "stock {} file has changed; variable `STOCK_PAM_D_SUDO` needs to be updated",
            PAM_D_SUDO_PATH
        );
    } else {
        // `PAMD_SUDO_PATH` should not exist in the base image
        Command::new("sh")
            .arg("-c")
            .arg(format!(
                "if [ -f {PAM_D_SUDO_PATH} ]; then exit 1; else exit 0; fi"
            ))
            .exec(&env)?
            .stdout()?;
    }

    let security_pam_env = Command::new("cat")
        .arg(SECURITY_PAM_ENV_PATH)
        .exec(&env)?
        .stdout()?;

    assert_eq!(
        STOCK_SECURITY_PAM_ENV,
        remove_comments_and_whitespace(&security_pam_env),
        "stock {} file has changed; variable `STOCK_SECURITY_PAM_ENV` needs to be updated",
        SECURITY_PAM_ENV_PATH
    );

    Ok(())
}

#[test]
#[ignore = "gh420"]
fn preserves_pam_env() -> Result<()> {
    let set_name = "SET_VAR";
    let set_value = "set";
    let default_name = "DEFAULT_VAR";
    let default_value = "default";
    let override_name = "OVERRIDE_VAR";
    let override_value = "override";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(PAM_D_SUDO_PATH, [STOCK_PAM_D_SUDO, PAM_D_SUDO_READENV])
        .file(ETC_ENVIRONMENT_PATH, format!("{set_name}={set_value}"))
        .file(
            SECURITY_PAM_ENV_PATH,
            [
                STOCK_SECURITY_PAM_ENV,
                &format!("{default_name} DEFAULT={default_value}"),
                &format!("{override_name} OVERRIDE={override_value}"),
            ],
        )
        .build()?;

    let stdout = Command::new("sudo").arg("env").exec(&env)?.stdout()?;
    let env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(set_value), env.get(set_name).copied());
    assert_eq!(Some(default_value), env.get(default_name).copied());
    assert_eq!(Some(override_value), env.get(override_name).copied());

    Ok(())
}

#[test]
#[ignore = "gh420"]
fn pam_env_has_precedence_over_callers_env() -> Result<()> {
    let set_name = "SET_VAR";
    let set_value = "set";
    let default_name = "DEFAULT_VAR";
    let default_value = "default";
    let override_name = "OVERRIDE_VAR";
    let override_value = "override";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(PAM_D_SUDO_PATH, [STOCK_PAM_D_SUDO, PAM_D_SUDO_READENV])
        .file(ETC_ENVIRONMENT_PATH, format!("{set_name}={set_value}"))
        .file(
            SECURITY_PAM_ENV_PATH,
            [
                STOCK_SECURITY_PAM_ENV,
                &format!("{default_name} DEFAULT={default_value}"),
                &format!("{override_name} OVERRIDE={override_value}"),
            ],
        )
        .build()?;

    let stdout = Command::new("env")
        .arg(format!("{set_name}=0"))
        .arg(format!("{default_name}=1"))
        .arg(format!("{override_name}=2"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(set_value), env.get(set_name).copied());
    assert_eq!(Some(default_value), env.get(default_name).copied());
    assert_eq!(Some(override_value), env.get(override_name).copied());

    Ok(())
}

fn env_list_has_precendece_over_pam_env(env_list: EnvList) -> Result<()> {
    let set_name = "SET_VAR";
    let set_value = "set";
    let default_name = "DEFAULT_VAR";
    let default_value = "default";
    let override_name = "OVERRIDE_VAR";
    let override_value = "override";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults {env_list} = \"{set_name} {default_name} {override_name}\""),
    ])
    .file(PAM_D_SUDO_PATH, [STOCK_PAM_D_SUDO, PAM_D_SUDO_READENV])
    .file(ETC_ENVIRONMENT_PATH, format!("{set_name}={set_value}"))
    .file(
        SECURITY_PAM_ENV_PATH,
        [
            STOCK_SECURITY_PAM_ENV,
            &format!("{default_name} DEFAULT={default_value}"),
            &format!("{override_name} OVERRIDE={override_value}"),
        ],
    )
    .build()?;

    let new_set_value = "0";
    let new_default_value = "1";
    let new_override_value = "2";
    let stdout = Command::new("env")
        .arg(format!("{set_name}={new_set_value}"))
        .arg(format!("{default_name}={new_default_value}"))
        .arg(format!("{override_name}={new_override_value}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(new_set_value), env.get(set_name).copied());
    assert_eq!(Some(new_default_value), env.get(default_name).copied());
    assert_eq!(Some(new_override_value), env.get(override_name).copied());

    Ok(())
}

#[test]
fn env_keep_has_precedence_over_pam_env() -> Result<()> {
    env_list_has_precendece_over_pam_env(EnvList::Keep)
}

#[test]
fn env_check_has_precedence_over_pam_env() -> Result<()> {
    env_list_has_precendece_over_pam_env(EnvList::Check)
}

#[test]
#[ignore = "gh420"]
fn var_rejected_by_env_check_falls_back_to_pam_env_value() -> Result<()> {
    let set_name = "SET_VAR";
    let set_value = "set";
    let default_name = "DEFAULT_VAR";
    let default_value = "default";
    let override_name = "OVERRIDE_VAR";
    let override_value = "override";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults env_check = \"{set_name} {default_name} {override_name}\""),
    ])
    .file(PAM_D_SUDO_PATH, [STOCK_PAM_D_SUDO, PAM_D_SUDO_READENV])
    .file(ETC_ENVIRONMENT_PATH, format!("{set_name}={set_value}"))
    .file(
        SECURITY_PAM_ENV_PATH,
        [
            STOCK_SECURITY_PAM_ENV,
            &format!("{default_name} DEFAULT={default_value}"),
            &format!("{override_name} OVERRIDE={override_value}"),
        ],
    )
    .build()?;

    let new_set_value = "%0";
    let new_default_value = "%1";
    let new_override_value = "%2";
    let stdout = Command::new("env")
        .arg(format!("{set_name}={new_set_value}"))
        .arg(format!("{default_name}={new_default_value}"))
        .arg(format!("{override_name}={new_override_value}"))
        .args(["sudo", "env"])
        .exec(&env)?
        .stdout()?;
    let env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(set_value), env.get(set_name).copied());
    assert_eq!(Some(default_value), env.get(default_name).copied());
    assert_eq!(Some(override_value), env.get(override_name).copied());

    Ok(())
}

#[test]
#[ignore = "gh420"]
fn default_and_override_pam_env_vars_are_parentheses_checked_but_set_vars_are_not() -> Result<()> {
    let set_name = "SET_VAR";
    let set_value = "() set";
    let default_name = "DEFAULT_VAR";
    let default_value = "() default";
    let override_name = "OVERRIDE_VAR";
    let override_value = "() override";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(PAM_D_SUDO_PATH, [STOCK_PAM_D_SUDO, PAM_D_SUDO_READENV])
        .file(ETC_ENVIRONMENT_PATH, format!("{set_name}={set_value}"))
        .file(
            SECURITY_PAM_ENV_PATH,
            [
                STOCK_SECURITY_PAM_ENV,
                &format!("{default_name} DEFAULT={default_value}"),
                &format!("{override_name} OVERRIDE={override_value}"),
            ],
        )
        .build()?;

    let stdout = Command::new("sudo").arg("env").exec(&env)?.stdout()?;
    let env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(set_value), env.get(set_name).copied());
    assert_eq!(None, env.get(default_name).copied());
    assert_eq!(None, env.get(override_name).copied());

    Ok(())
}

#[test]
#[ignore = "gh420"]
fn pam_env_vars_are_not_env_checked() -> Result<()> {
    let set_name = "SET_VAR";
    let set_value = "%set";
    let default_name = "DEFAULT_VAR";
    let default_value = "%default";
    let override_name = "OVERRIDE_VAR";
    let override_value = "%override";
    let env = Env([
        SUDOERS_ALL_ALL_NOPASSWD,
        &format!("Defaults env_check = \"{set_name} {default_name} {override_name}\""),
    ])
    .file(PAM_D_SUDO_PATH, [STOCK_PAM_D_SUDO, PAM_D_SUDO_READENV])
    .file(ETC_ENVIRONMENT_PATH, format!("{set_name}={set_value}"))
    .file(
        SECURITY_PAM_ENV_PATH,
        [
            STOCK_SECURITY_PAM_ENV,
            &format!("{default_name} DEFAULT={default_value}"),
            &format!("{override_name} OVERRIDE={override_value}"),
        ],
    )
    .build()?;

    let stdout = Command::new("sudo").arg("env").exec(&env)?.stdout()?;
    let env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(set_value), env.get(set_name).copied());
    assert_eq!(Some(default_value), env.get(default_name).copied());
    assert_eq!(Some(override_value), env.get(override_name).copied());

    Ok(())
}
