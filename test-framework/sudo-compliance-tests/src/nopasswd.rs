//! Scenarios where a password does not need to be provided

use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_ROOT_ALL, USERNAME};

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
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .args(["-u", USERNAME, "true"])
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}

#[test]
fn nopasswd_tag() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) NOPASSWD: ALL"))
        .user(USERNAME)
        .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}
