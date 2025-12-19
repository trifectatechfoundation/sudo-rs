use sudo_test::{Command, Env};

use crate::SUDOERS_ALL_ALL_NOPASSWD;

#[test]
fn runs_in_background() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build();

    Command::new("sudo")
        .args([
            "-b",
            "sh",
            "-c",
            "touch /tmp/barrier1; until [ -f /tmp/barrier2 ]; do sleep 0.1; done; touch /tmp/barrier3",
        ])
        .output(&env)
        .assert_success();

    Command::new("sh")
        .args([
            "-c",
            "until [ -f /tmp/barrier1 ]; do sleep 0.1; done
             touch /tmp/barrier2
             until [ -f /tmp/barrier3 ]; do sleep 0.1; done",
        ])
        .output(&env)
        .assert_success();
}
