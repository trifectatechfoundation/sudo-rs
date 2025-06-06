use sudo_test::{Command, Env};

use crate::LONGEST_HOSTNAME;

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../../snapshots/sudoers/host_list",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn given_specific_hostname_then_sudo_from_said_hostname_is_allowed() {
    let hostname = "container";
    let env = Env(format!("ALL {hostname} = (ALL:ALL) ALL"))
        .hostname(hostname)
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn given_specific_hostname_then_sudo_from_different_hostname_is_rejected() {
    let env = Env("ALL remotehost = (ALL:ALL) ALL")
        .hostname("container")
        .build();

    let output = Command::new("sudo").arg("true").output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "I'm sorry root. I'm afraid I can't do that");
    }
}

#[test]
fn different() {
    let env = Env("ALL remotehost, container = (ALL:ALL) ALL")
        .hostname("container")
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn repeated() {
    let env = Env("ALL container, container = (ALL:ALL) ALL")
        .hostname("container")
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn negation_rejects() {
    let env = Env("ALL remotehost, !container = (ALL:ALL) ALL")
        .hostname("container")
        .build();

    let output = Command::new("sudo").arg("true").output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "I'm sorry root. I'm afraid I can't do that");
    }
}

#[test]
fn double_negative_is_positive() {
    let env = Env("ALL !!container = (ALL:ALL) ALL")
        .hostname("container")
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn longest_hostname() {
    let env = Env(format!("ALL {LONGEST_HOSTNAME} = (ALL:ALL) ALL"))
        .hostname(LONGEST_HOSTNAME)
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}
