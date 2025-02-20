use sudo_test::User;
use sudo_test::{Command, Env};

use crate::{helpers, Result, USERNAME};

#[test]
fn rootpw_can_be_per_host_correct_host() -> Result<()> {
    const PASSWORD: &str = "passw0rd";
    const ROOT_PASSWORD: &str = "r00t";

    let env = Env(format!(
        "Defaults@container rootpw
        Defaults passwd_tries=1
        {USERNAME} ALL=(ALL:ALL) ALL"
    ))
    .user_password("root", ROOT_PASSWORD)
    .user(User(USERNAME).password(PASSWORD))
    .hostname("container")
    .build()?;

    // User password is not accepted when rootpw is enabled
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env)?;
    assert!(!output.status().success());

    // Root password is accepted when rootpw is enabled
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env)?;
    assert!(output.status().success());

    Ok(())
}

#[test]
fn rootpw_can_be_per_host_incorrect_host() -> Result<()> {
    const PASSWORD: &str = "passw0rd";
    const ROOT_PASSWORD: &str = "r00t";

    let env = Env(format!(
        "Defaults@container rootpw
        Defaults passwd_tries=1
        {USERNAME} ALL=(ALL:ALL) ALL"
    ))
    .user_password("root", ROOT_PASSWORD)
    .user(User(USERNAME).password(PASSWORD))
    .hostname("c0ntainer")
    .build()?;

    // Root password is not accepted when rootpw is enabled
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env)?;
    assert!(!output.status().success());

    // User password is accepted when rootpw is enabled
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env)?;
    assert!(output.status().success());

    Ok(())
}

#[test]
fn rootpw_can_be_per_user() -> Result<()> {
    const PASSWORD: &str = "passw0rd";
    const ROOT_PASSWORD: &str = "r00t";

    let env = Env(format!(
        "Defaults:{USERNAME} rootpw
        Defaults passwd_tries=1
        {USERNAME} ALL=(ALL:ALL) ALL"
    ))
    .user_password("root", ROOT_PASSWORD)
    .user(User(USERNAME).password(PASSWORD))
    .user(User("other").password("otherpwd"))
    .build()?;

    // Root password is not accepted for other user
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S true"))
        .as_user("other")
        .output(&env)?;
    assert!(!output.status().success());

    // Root password is accepted for user
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env)?;
    assert!(output.status().success());

    Ok(())
}

#[test]
fn rootpw_can_be_per_runas() -> Result<()> {
    const PASSWORD: &str = "passw0rd";
    const ROOT_PASSWORD: &str = "r00t";

    let env = Env(format!(
        "Defaults>ALL,!other rootpw
        Defaults passwd_tries=1
        {USERNAME} ALL=(ALL:ALL) ALL"
    ))
    .user_password("root", ROOT_PASSWORD)
    .user(User(USERNAME).password(PASSWORD))
    .user(User("other").password("pwd"))
    .build()?;

    // Root password is not accepted for "run as other"
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S -u other true"))
        .as_user(USERNAME)
        .output(&env)?;
    assert!(!output.status().success());

    // Root password is accepted for any other runas
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S true"))
        .as_user(USERNAME)
        .output(&env)?;
    assert!(output.status().success());

    Ok(())
}

#[test]
fn rootpw_can_be_per_general_command() -> Result<()> {
    const PASSWORD: &str = "passw0rd";
    const ROOT_PASSWORD: &str = "r00t";

    let env = Env(format!(
        "Defaults!/usr/bin/tr* rootpw
        Defaults passwd_tries=1
        {USERNAME} ALL=(ALL:ALL) ALL"
    ))
    .user_password("root", ROOT_PASSWORD)
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    // Root password is not accepted for 'whoami'
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S whoami"))
        .as_user(USERNAME)
        .output(&env)?;
    assert!(!output.status().success());

    // Root password is accepted for 'true'
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S true args"))
        .as_user(USERNAME)
        .output(&env)?;
    assert!(output.status().success());

    Ok(())
}

#[test]
fn rootpw_can_be_per_command_w_args() -> Result<()> {
    const PASSWORD: &str = "passw0rd";
    const ROOT_PASSWORD: &str = "r00t";

    let env = Env(format!(
        "Cmnd_Alias TRUE=/usr/bin/true ignored
         Defaults!TRUE rootpw
         Defaults passwd_tries=1
         {USERNAME} ALL=(ALL:ALL) ALL"
    ))
    .user_password("root", ROOT_PASSWORD)
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    // Root password is not accepted for 'whoami'
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S true bla"))
        .as_user(USERNAME)
        .output(&env)?;
    assert!(!output.status().success());

    // Root password is accepted for 'true'
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {ROOT_PASSWORD} | sudo -S true ignored"))
        .as_user(USERNAME)
        .output(&env)?;
    assert!(output.status().success());

    Ok(())
}

//note: we don't repeat all of the above combinations, the following tests
//focus on the peculiar behaviour of 'secure_path'

#[test]
fn securepath_can_be_per_user() -> Result<()> {
    const PASSWORD: &str = "passw0rd";

    let env = Env(format!(
        "Defaults secure_path=\"/usr/bin\"
        Defaults:{USERNAME} secure_path=\"/user/\"
        ALL ALL=NOPASSWD: ALL"
    ))
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    // Command is not found (/root/true does not exist)
    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?;
    assert!(!output.status().success());
    assert_contains!(output.stderr(), "command not found");

    // Commmand is found in the usual location
    let output = Command::new("sudo").arg("true").output(&env)?;
    assert!(output.status().success());

    Ok(())
}

#[test]
fn securepath_can_be_per_command() -> Result<()> {
    let env = Env("Defaults secure_path=\"/usr/bin\"
        Defaults!/usr/bin/env secure_path=\"/user\"
        ALL ALL=NOPASSWD: ALL")
    .build()?;

    // Command *is* found, but adopts the secure_path
    let output = Command::new("sudo").arg("env").output(&env)?;
    assert!(output.status().success());

    let stdout = output.stdout()?;
    let env_vars = helpers::parse_env_output(&stdout)?;
    assert_eq!(env_vars["PATH"], "/user");

    Ok(())
}
