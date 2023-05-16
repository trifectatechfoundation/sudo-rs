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

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr(), "3 incorrect password attempts");
    }

    Ok(())
}

// this is a PAM security feature
#[test]
#[ignore]
fn retry_is_not_allowed_immediately() -> Result<()> {
    // expected format: `{minutes}:{seconds}.{centiseconds}`
    const BAD_TIME_FORMAT: &str = "bad `/usr/bin/time` format";

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

    let (_minutes, second_centis) = output.rsplit_once(':').expect(BAD_TIME_FORMAT);
    let (seconds, centis) = second_centis.split_once('.').expect(BAD_TIME_FORMAT);
    assert_eq!(2, centis.len(), "{BAD_TIME_FORMAT}");

    let total_millis = seconds.parse::<u32>().expect(BAD_TIME_FORMAT) * 1_000
        + centis.parse::<u32>().expect(BAD_TIME_FORMAT) * 10;

    // by default, this should be around 2 seconds
    assert!(total_millis >= 1500);

    Ok(())
}
