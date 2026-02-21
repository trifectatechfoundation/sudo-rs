//! Test the Cmnd_Spec component of the user specification: <user> ALL=(ALL:ALL) <cmnd_spec>

use sudo_test::{BIN_LS, BIN_TRUE, Command, ETC_SUDOERS, Env, TextFile};

use crate::USERNAME;

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![
                (r"[[:xdigit:]]{12}", "[host]"),
                (ETC_SUDOERS, "/etc/sudoers"),
            ],
            prepend_module_to_snapshot => false,
            snapshot_path => "../../snapshots/sudoers/cmnd",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn given_specific_command_then_that_command_is_allowed() {
    let env = Env(format!("ALL ALL=(ALL:ALL) {BIN_TRUE}")).build();

    Command::new("sudo")
        .arg(BIN_TRUE)
        .output(&env)
        .assert_success()
}

#[test]
fn given_specific_command_then_other_command_is_not_allowed() {
    let env = Env(format!("ALL ALL=(ALL:ALL) {BIN_LS}")).build();

    let output = Command::new("sudo").arg(BIN_TRUE).output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "I'm sorry root. I'm afraid I can't do that");
    }
}

#[test]
fn given_specific_command_with_nopasswd_tag_then_no_password_auth_is_required() {
    let env = Env(format!("ALL ALL=(ALL:ALL) NOPASSWD: {BIN_TRUE}"))
        .user(USERNAME)
        .build();

    Command::new("sudo")
        .arg(BIN_TRUE)
        .as_user(USERNAME)
        .output(&env)
        .assert_success()
}

#[test]
fn command_specified_not_by_absolute_path_is_rejected() {
    let env = Env("ALL ALL=(ALL:ALL) true").build();

    let output = Command::new("sudo").arg(BIN_TRUE).output(&env);

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
    let env = Env(format!("ALL ALL=(ALL:ALL) {BIN_TRUE}, {BIN_LS}")).build();

    Command::new("sudo")
        .arg(BIN_TRUE)
        .output(&env)
        .assert_success();

    let output = Command::new("sudo").args([BIN_LS, "/root"]).output(&env);

    output.assert_success();
}

// it applies not only to the command is next to but to all commands that follow
#[test]
fn nopasswd_is_sticky() {
    let env = Env(format!("ALL ALL=(ALL:ALL) NOPASSWD: {BIN_LS}, {BIN_TRUE}"))
        .user(USERNAME)
        .build();

    Command::new("sudo")
        .arg(BIN_TRUE)
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn repeated() {
    let env = Env(format!("ALL ALL=(ALL:ALL) {BIN_TRUE}, {BIN_TRUE}")).build();

    Command::new("sudo")
        .arg(BIN_TRUE)
        .output(&env)
        .assert_success();
}

#[test]
fn nopasswd_override() {
    let env = Env(format!(
        "ALL ALL=(ALL:ALL) {BIN_TRUE}, NOPASSWD: {BIN_TRUE}"
    ))
    .user(USERNAME)
    .build();

    Command::new("sudo")
        .arg(BIN_TRUE)
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn runas_override() {
    let env = Env(format!(
        "ALL ALL = (root) {BIN_LS}, ({USERNAME}) {BIN_TRUE}"
    ))
    .user(USERNAME)
    .build();

    let output = Command::new("sudo").args([BIN_LS, "/root"]).output(&env);

    output.assert_success();

    let output = Command::new("sudo")
        .args(["-u", USERNAME, BIN_LS])
        .output(&env);

    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("user root is not allowed to execute '{BIN_LS}' as ferris")
    } else {
        "I'm sorry root. I'm afraid I can't do that".to_owned()
    };
    assert_contains!(output.stderr(), diagnostic);

    Command::new("sudo")
        .args(["-u", "ferris", BIN_TRUE])
        .output(&env)
        .assert_success();

    let output = Command::new("sudo").arg(BIN_TRUE).output(&env);

    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("user root is not allowed to execute '{BIN_TRUE}' as root")
    } else {
        "I'm sorry root. I'm afraid I can't do that".to_owned()
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn runas_override_repeated_cmnd_means_runas_union() {
    let env = Env(format!(
        "ALL ALL = (root) {BIN_TRUE}, ({USERNAME}) {BIN_TRUE}"
    ))
    .user(USERNAME)
    .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();

    Command::new("sudo")
        .args(["-u", USERNAME, "true"])
        .output(&env)
        .assert_success();
}

#[test]
fn given_directory_then_commands_in_it_are_allowed() {
    let env = Env("ALL ALL=(ALL:ALL) /usr/bin/").build();

    Command::new("sudo")
        .arg("/usr/bin/true")
        .output(&env)
        .assert_success();
}

#[test]
fn given_directory_then_commands_in_its_subdirectories_are_not_allowed() {
    let env = Env("ALL ALL=(ALL:ALL) /usr/").build();

    let output = Command::new("sudo").arg("/usr/bin/true").output(&env);

    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "user root is not allowed to execute '/usr/bin/true' as root"
    } else {
        "I'm sorry root. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn wildcards_are_allowed_for_dir() {
    let env = Env("ALL ALL=(ALL:ALL) /usr/*/true").build();

    Command::new("sudo")
        .arg("/usr/bin/true")
        .output(&env)
        .assert_success();
}

#[test]
fn wildcards_are_allowed_for_file() {
    let env = Env("ALL ALL=(ALL:ALL) /usr/bin/*").build();

    Command::new("sudo")
        .arg("/usr/bin/true")
        .output(&env)
        .assert_success();
}

// due to frequent misusage ("sudo: you are doing it wrong"), we explicitly don't support this
#[test]
#[ignore = "wontfix"]
fn wildcards_are_allowed_for_args() {
    let env = Env(format!("ALL ALL=(ALL:ALL) {BIN_TRUE} /root/*")).build();

    Command::new("sudo")
        .arg("true")
        .args(["/root/", "hello", "world"])
        .output(&env)
        .assert_success();
}

#[test]
fn arguments_can_be_supplied() {
    for supplied_arg in ["", "*"] {
        let env = Env(format!("ALL ALL=(ALL:ALL) {BIN_TRUE} {supplied_arg}")).build();

        Command::new("sudo")
            .arg("true")
            .args(["/root/", "hello", "world"])
            .output(&env)
            .assert_success();

        Command::new("sudo")
            .arg("true")
            .arg("foo")
            .output(&env)
            .assert_success();
    }
}

#[test]
fn arguments_can_be_forced() {
    for supplied_arg in ["hello world", "hello *"] {
        let env = Env(format!("ALL ALL=(ALL:ALL) {BIN_TRUE} {supplied_arg}")).build();

        Command::new("sudo")
            .arg("true")
            .args(["hello", "world"])
            .output(&env)
            .assert_success();

        let output = Command::new("sudo")
            .arg("true")
            .args(["/root/", "hello", "world"])
            .output(&env);

        output.assert_exit_code(1);

        let diagnostic = if sudo_test::is_original_sudo() {
            format!("user root is not allowed to execute '{BIN_TRUE} /root/ hello world' as root")
        } else {
            "I'm sorry root. I'm afraid I can't do that".to_owned()
        };
        assert_contains!(output.stderr(), diagnostic);
    }
}

#[test]
fn arguments_can_be_forbidden() {
    for supplied_arg in ["\"\"", "/root/"] {
        let env = Env(format!("ALL ALL=(ALL:ALL) {BIN_TRUE} {supplied_arg}")).build();

        let output = Command::new("sudo")
            .arg("true")
            .args(["/root/", "hello", "world"])
            .output(&env);

        output.assert_exit_code(1);

        let diagnostic = if sudo_test::is_original_sudo() {
            format!("user root is not allowed to execute '{BIN_TRUE} /root/ hello world' as root")
        } else {
            "I'm sorry root. I'm afraid I can't do that".to_owned()
        };
        assert_contains!(output.stderr(), diagnostic);
    }
}

#[test]
fn wildcards_dont_cross_directory_boundaries() {
    let env = Env("ALL ALL=(ALL:ALL) /usr/*/foo")
        .directory("/usr/bin/sub")
        .file("/usr/bin/sub/foo", TextFile("").chown("root").chmod("777"))
        .build();

    let output = Command::new("sudo").arg("/usr/bin/sub/foo").output(&env);

    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "user root is not allowed to execute '/usr/bin/sub/foo' as root"
    } else {
        "I'm sorry root. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);
}
