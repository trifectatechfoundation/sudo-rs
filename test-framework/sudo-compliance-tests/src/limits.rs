use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

#[test]
#[ignore = "gh644"]
fn etc_security_limits_rules_apply_according_to_the_target_user() -> Result<()> {
    let target_user = "ghost";
    let original = "2048";
    let expected = "1024";
    let limits = format!(
        "{USERNAME} hard locks {original}
{target_user} hard locks {expected}"
    );
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file("/etc/security/limits.d/50-test.conf", limits)
        .user(USERNAME)
        .user(target_user)
        .build()?;

    // this appears to ignore the `limits` rules, perhaps because of docker
    // in any case, the assertion below and the rule above should be enough to check that the
    // *target* user's, and not the invoking user's, limits apply when sudo is involved
    // let normal_limit = Command::new("bash")
    //     .args(["-c", "ulimit -x"])
    //     .as_user(USERNAME)
    //     .output(&env)?
    //     .stdout()?;

    // assert_eq!(original, normal_limit);

    // check that limits apply even when root is the invoking user
    let users = ["root", USERNAME];
    for invoking_user in users {
        let sudo_limit = Command::new("sudo")
            .args(["-u", target_user, "bash", "-c", "ulimit -x"])
            .as_user(invoking_user)
            .output(&env)?
            .stdout()?;

        assert_eq!(expected, sudo_limit);
    }

    Ok(())
}

// see `man sudoers`; 'SUDOERS FORMAT' section; 'Resource limits' subsection
//
// "The one exception to this is the core dump file size, which is set by sudoers to 0 by default."
#[test]
#[ignore = "gh644"]
fn core_file_size_is_set_to_zero() -> Result<()> {
    let users = ["root", USERNAME];

    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build()?;
    for invoking_user in users {
        let normal_limit = Command::new("sh")
            .args(["-c", "ulimit -c"])
            .as_user(invoking_user)
            .output(&env)?
            .stdout()?;

        assert_eq!("unlimited", normal_limit);

        let sudo_limit = Command::new("sudo")
            .args(["sh", "-c", "ulimit -c"])
            .as_user(invoking_user)
            .output(&env)?
            .stdout()?;

        assert_eq!("0", sudo_limit);
    }

    Ok(())
}

#[test]
#[ignore = "gh644"]
fn cannot_override_the_default_core_file_size_with_a_limits_file() -> Result<()> {
    let target_user = "ghost";
    let rule = "1024";
    let limits = format!("{target_user} hard core {rule}");
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file("/etc/security/limits.d/50-test.conf", limits)
        .user(USERNAME)
        .user(target_user)
        .build()?;

    // check that limits apply even when root is the invoking user
    let users = ["root", USERNAME];
    for invoking_user in users {
        dbg!(invoking_user);
        let sudo_limit = Command::new("sudo")
            .args(["-u", target_user, "bash", "-c", "ulimit -c"])
            .as_user(invoking_user)
            .output(&env)?
            .stdout()?;

        assert_eq!("0", sudo_limit);
    }

    Ok(())
}
