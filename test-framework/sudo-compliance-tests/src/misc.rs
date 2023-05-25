use sudo_test::{Command, Env};

use crate::{
    helpers, Result, SUDOERS_ALL_ALL_NOPASSWD, SUDOERS_ROOT_ALL_NOPASSWD, SUDO_RS_IS_UNSTABLE,
    USERNAME,
};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "snapshots/misc",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn user_not_in_passwd_database_cannot_use_sudo() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user_id(1000)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "user `current user' not found");
    }

    Ok(())
}

#[test]
#[ignore]
fn closes_open_file_descriptors() -> Result<()> {
    let script_path = "/tmp/script.bash";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(
            script_path,
            include_str!("misc/read-parents-open-file-descriptor.bash"),
        )
        .build()?;

    let output = Command::new("bash").arg(script_path).exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    assert_contains!(output.stderr(), "42: Bad file descriptor");

    Ok(())
}

#[test]
#[ignore]
fn sudo_binary_lacks_setuid_flag() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    Command::new("chmod")
        .args(["0755", "/usr/bin/sudo"])
        .exec(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    assert_contains!(
        output.stderr(),
        "sudo must be owned by uid 0 and have the setuid bit set"
    );

    Ok(())
}

#[test]
#[ignore]
fn sudo_binary_is_not_owned_by_root() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;

    Command::new("chown")
        .args([USERNAME, "/usr/bin/sudo"])
        .exec(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    assert_contains!(
        output.stderr(),
        "sudo must be owned by uid 0 and have the setuid bit set"
    );

    Ok(())
}

// see 'environment' section in `man sudo`
// "SUDO_PS1: If set, PS1 will be set to its value for the program being run."
#[test]
fn ps1_env_var_is_set_when_sudo_ps1_is_set() -> Result<()> {
    let ps1 = "abc";
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").exec(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").exec(&env)?.stdout()?;

    // run sudo in an empty environment
    let stdout = Command::new("env")
        .args(["-i", SUDO_RS_IS_UNSTABLE])
        .arg(format!("SUDO_PS1={ps1}"))
        .args([&sudo_abs_path, &env_abs_path])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert_eq!(Some(ps1), sudo_env.get("PS1").copied());
    assert!(sudo_env.get("SUDO_PS1").is_none());

    Ok(())
}

#[test]
fn ps1_env_var_is_not_set_when_sudo_ps1_is_set_and_flag_login_is_used() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL_NOPASSWD).build()?;

    let sudo_abs_path = Command::new("which").arg("sudo").exec(&env)?.stdout()?;
    let env_abs_path = Command::new("which").arg("env").exec(&env)?.stdout()?;

    // run sudo in an empty environment
    let stdout = Command::new("env")
        .args(["-i", SUDO_RS_IS_UNSTABLE])
        .arg("SUDO_PS1=abc")
        .args([&sudo_abs_path, "-i", &env_abs_path])
        .exec(&env)?
        .stdout()?;
    let sudo_env = helpers::parse_env_output(&stdout)?;

    assert!(sudo_env.get("PS1").is_none());
    assert!(sudo_env.get("SUDO_PS1").is_none());

    Ok(())
}
