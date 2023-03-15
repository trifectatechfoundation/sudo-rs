//! Scenarios where a password does not need to be provided

use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_ROOT_ALL};

// NOTE all these tests assume that the invoking user passes the sudoers file 'User_List' criteria

// man sudoers > User Authentication:
// "A password is not required if the invoking user is root"
#[ignore]
#[test]
fn user_is_root() -> Result<()> {
    let env = Env(SUDOERS_ROOT_ALL).build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}

// man sudoers > User Authentication:
// "A password is not required if (..) the target user is the same as the invoking user"
#[ignore]
#[test]
fn user_as_themselves() -> Result<()> {
    let username = "ferris";
    let env = Env(format!("{username}    ALL=(ALL:ALL) ALL"))
        .user(username)
        .build()?;

    Command::new("sudo")
        .args(["-u", username, "true"])
        .as_user(username)
        .exec(&env)?
        .assert_success()
}

#[test]
fn nopasswd_tag() -> Result<()> {
    let username = "ferris";
    let env = Env(format!("{username}    ALL=(ALL:ALL) NOPASSWD: ALL"))
        .user(username)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(username)
        .exec(&env)?
        .assert_success()
}
