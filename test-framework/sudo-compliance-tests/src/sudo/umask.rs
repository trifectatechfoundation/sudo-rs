use sudo_test::{Command, Env};

use crate::SUDOERS_ALL_ALL_NOPASSWD;

fn test_umask(config: &str, user_umask: &str, target_umask: &str) {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, config]).build();

    let output = Command::new("sh")
        .args(["-c", &format!("umask {user_umask}; sudo sh -c umask")])
        .output(&env);
    output.assert_success();

    assert_eq!(output.stdout(), target_umask);
}

#[test]
fn umask_unchanged() {
    test_umask("Defaults umask=0777", "0123", "0123");
    test_umask("Defaults !umask", "0123", "0123");
}

#[test]
fn stricter_umask_respected() {
    test_umask("Defaults umask=0776", "0022", "0776");
}

#[test]
fn overlapping_umask_unioned() {
    test_umask("Defaults umask=0770", "0022", "0772");
}

#[test]
fn looser_umask_unchanged() {
    test_umask("Defaults umask=0000", "0022", "0022");
}

#[test]
fn umask_override() {
    test_umask(
        "Defaults umask=0700\nDefaults umask_override",
        "0022",
        "0700",
    );
}

#[test]
fn umask_override_0777() {
    test_umask(
        "Defaults umask=0777\nDefaults umask_override",
        "0022",
        "0022",
    );
}
