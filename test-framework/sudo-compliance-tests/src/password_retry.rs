use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

#[test]
#[ignore]
fn can_retry_password() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "(echo wrong-password; echo {PASSWORD}) | sudo -S true"
        ))
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}

#[test]
#[ignore]
fn three_retries_allowed_by_default() -> Result<()> {
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "(for i in $(seq 1 3); do echo wrong-password; done; echo {PASSWORD}) | sudo -S true"
        ))
        .as_user(USERNAME)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();

    let diagnostic = if sudo_test::is_original_sudo() {
        "3 incorrect password attempts"
    } else {
        "Authentication failure"
    };
    assert_contains!(output.stderr(), diagnostic);

    let password_prompt = if sudo_test::is_original_sudo() {
        "password for ferris:"
    } else {
        "Password:"
    };

    let num_password_prompts = stderr
        .lines()
        .filter(|line| line.contains(password_prompt))
        .count();

    assert_eq!(3, num_password_prompts);

    Ok(())
}

#[test]
#[ignore]
fn defaults_passwd_tries() -> Result<()> {
    let env = Env(format!(
        "{USERNAME} ALL=(ALL:ALL) ALL
Defaults passwd_tries=2"
    ))
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "(for i in $(seq 1 2); do echo wrong-password; done; echo {PASSWORD}) | sudo -S true"
        ))
        .as_user(USERNAME)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    let diagnostic = if sudo_test::is_original_sudo() {
        "2 incorrect password attempts"
    } else {
        "Authentication failure"
    };
    assert_contains!(stderr, diagnostic);

    let password_prompt = if sudo_test::is_original_sudo() {
        "password for ferris:"
    } else {
        "Password:"
    };

    let num_password_prompts = stderr
        .lines()
        .filter(|line| line.contains(password_prompt))
        .count();

    assert_eq!(2, num_password_prompts);

    Ok(())
}

// this is a PAM security feature
#[test]
#[ignore]
fn retry_is_not_allowed_immediately() -> Result<()> {
    let path = "/tmp/measurement";

    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    Command::new("/usr/bin/time")
        .args(["-f", "%E"])
        .args(["-o", path])
        .args(["sh", "-c"])
        .arg(format!(
            "(echo wrong-password; echo {PASSWORD}) | sudo -S true"
        ))
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()?;

    let output = Command::new("cat").arg(path).exec(&env)?.stdout()?;

    let total_millis = parse_usr_bin_time_output(&output);

    // by default, this should be around 2 seconds
    assert!(total_millis >= 1500);

    Ok(())
}

#[test]
#[ignore]
fn can_control_retry_delay_using_pam() -> Result<()> {
    let check_env = Env("").build()?;
    let common_auth = Command::new("cat")
        .arg("/etc/pam.d/common-auth")
        .exec(&check_env)?
        .stdout()?;
    let common_auth = common_auth
        .lines()
        .filter(|line| !line.trim_start().starts_with('#') && !line.trim().is_empty())
        .collect::<Vec<&str>>()
        .join("\n");
    assert_eq!(
        "auth\t[success=1 default=ignore]\tpam_unix.so nullok
auth\trequisite\t\t\tpam_deny.so
auth\trequired\t\t\tpam_permit.so",
        common_auth,
        "the stock /etc/pam.d/common-auth file has changed; this test needs to be updated"
    );

    // increase the retry delay from 2 seconds to 5
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .file(
            "/etc/pam.d/common-auth",
            "auth optional pam_faildelay.so delay=5000000
auth [success=1 default=ignore] pam_unix.so nullok nodelay
auth requisite pam_deny.so
auth required pam_permit.so",
        )
        .build()?;

    let path = "/tmp/measurement";
    Command::new("/usr/bin/time")
        .args(["-f", "%E"])
        .args(["-o", path])
        .args(["sh", "-c"])
        .arg(format!(
            "(echo wrong-password; echo {PASSWORD}) | sudo -S true"
        ))
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()?;

    let output = Command::new("cat").arg(path).exec(&env)?.stdout()?;

    let total_millis = parse_usr_bin_time_output(&output);

    assert!(total_millis >= 4500);

    Ok(())
}

fn parse_usr_bin_time_output(output: &str) -> u32 {
    // expected format: `{minutes}:{seconds}.{centiseconds}`
    const BAD_TIME_FORMAT: &str = "bad `/usr/bin/time` format";

    let (_minutes, second_centis) = output.rsplit_once(':').expect(BAD_TIME_FORMAT);
    let (seconds, centis) = second_centis.split_once('.').expect(BAD_TIME_FORMAT);
    assert_eq!(2, centis.len(), "{BAD_TIME_FORMAT}");

    seconds.parse::<u32>().expect(BAD_TIME_FORMAT) * 1_000
        + centis.parse::<u32>().expect(BAD_TIME_FORMAT) * 10
}
