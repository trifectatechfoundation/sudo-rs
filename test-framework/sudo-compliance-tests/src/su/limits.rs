use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

#[test]
fn etc_security_limits_rules_apply_according_to_the_target_user() -> Result<()> {
    let target_user = "ghost";
    let original = "2048";
    let expected = "1024";
    let limits = format!(
        "{USERNAME} hard locks {original}
{target_user} hard locks {expected}"
    );
    let env = Env("")
        .file("/etc/security/limits.d/50-test.conf", limits)
        .user(USERNAME)
        .user(User(target_user).password(PASSWORD).shell("/bin/bash"))
        .build()?;

    // this appears to ignore the `limits` rules, perhaps because of docker
    // in any case, the assertion below and the rule above should be enough to check that the
    // *target* user's, and not the invoking user's, limits apply when su is involved
    // let normal_limit = Command::new("bash")
    //     .args(["-c", "ulimit -x"])
    //     .as_user(USERNAME)
    //     .output(&env)?
    //     .stdout()?;

    // assert_eq!(original, normal_limit);

    // check that limits apply even when root is the invoking user
    let users = ["root", USERNAME];
    for invoking_user in users {
        let su_limit = Command::new("su")
            .args(["-c", "ulimit -x", target_user])
            .stdin(PASSWORD)
            .as_user(invoking_user)
            .output(&env)?
            .stdout()?;

        assert_eq!(expected, su_limit);
    }

    Ok(())
}
