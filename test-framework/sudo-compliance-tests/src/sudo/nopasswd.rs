//! Scenarios where a password does not need to be provided

use sudo_test::{Command, Env, User, BIN_LS, BIN_TRUE};

use crate::{GROUPNAME, SUDOERS_NO_LECTURE, SUDOERS_ROOT_ALL, USERNAME};

// NOTE all these tests assume that the invoking user passes the sudoers file 'User_List' criteria

// man sudoers > User Authentication:
// "A password is not required if the invoking user is root"
#[test]
fn user_is_root() {
    let env = Env(SUDOERS_ROOT_ALL).build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

// man sudoers > User Authentication:
// "A password is not required if (..) the target user is the same as the invoking user"
#[test]
fn user_as_themselves() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(USERNAME)
        .build();

    Command::new("sudo")
        .args(["-u", USERNAME, "true"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn user_as_their_own_group() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .group(GROUPNAME)
        .user(User(USERNAME).secondary_group(GROUPNAME))
        .build();

    Command::new("sudo")
        .args(["-g", GROUPNAME, "true"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn nopasswd_tag() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) NOPASSWD: ALL"))
        .user(USERNAME)
        .build();

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn nopasswd_tag_for_command() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) NOPASSWD: {BIN_TRUE}"))
        .user(USERNAME)
        .build();

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn run_sudo_l_flag_without_pwd_if_one_nopasswd_is_set() {
    let env = Env(format!(
        "ALL ALL=(ALL:ALL) NOPASSWD: {BIN_TRUE}, PASSWD: {BIN_LS}"
    ))
    .user(USERNAME)
    .build();

    let output = Command::new("sudo")
        .arg("-l")
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();

    let actual = output.stdout();
    assert_contains!(
        actual,
        format!("User {USERNAME} may run the following commands")
    );
}

#[test]
fn run_sudo_v_flag_without_pwd_if_nopasswd_is_set_for_all_users_entries() {
    let env = Env(format!(
        "{USERNAME}    ALL=(ALL:ALL) NOPASSWD: {BIN_TRUE}, {BIN_LS}"
    ))
    .user(USERNAME)
    .build();

    Command::new("sudo")
        .arg("-v")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn v_flag_without_pwd_fails_if_nopasswd_is_not_set_for_all_users_entries() {
    let env = Env([
        format!("ALL ALL=(ALL:ALL) NOPASSWD: {BIN_TRUE}, PASSWD: {BIN_LS}"),
        SUDOERS_NO_LECTURE.to_owned(),
    ])
    .user(USERNAME)
    .build();

    let output = Command::new("sudo")
        .args(["-S", "-v"])
        .as_user(USERNAME)
        .output(&env);

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        if cfg!(not(target_os = "linux")) {
            assert_contains!(
                stderr,
                "Password: \nsudo: no password was provided\nsudo: a password is required"
                    .to_owned()
            );
        } else {
            assert_contains!(
                stderr,
                format!("[sudo] password for {USERNAME}: \nsudo: no password was provided\nsudo: a password is required")
            );
        }
    } else {
        assert_contains!(stderr, "Password is required");
    }
}
