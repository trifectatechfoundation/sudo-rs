use sudo_test::{Command, Env};

use crate::Result;

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/sudoers/host_alias",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn host_alias_works() -> Result<()> {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "ALL SERVERS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn host_alias_can_contain_underscore_and_digits() -> Result<()> {
    let env = Env([
        "Host_Alias UNDER_SCORE123 = ALL",
        "ALL UNDER_SCORE123 = (ALL:ALL) NOPASSWD: /usr/bin/true",
    ])
    .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn host_alias_cannot_start_with_underscore() -> Result<()> {
    let env = Env([
        "Host_Alias _FOO = ALL",
        "ALL ALL = (ALL:ALL) NOPASSWD: /usr/bin/true",
        "ALL _FOO = (ALL:ALL) PASSWD: ALL",
    ])
    .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn host_alias_negation() -> Result<()> {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "ALL !SERVERS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn host_alias_double_negation() -> Result<()> {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "ALL !!SERVERS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn combined_host_aliases() -> Result<()> {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = OTHERHOSTS, !SERVERS",
        "ALL WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("foo")
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;
    assert!(output.status().success());

    let second_env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = OTHERHOSTS, !SERVERS",
        "ALL WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build()?;

    let second_output = Command::new("sudo").arg("true").output(&second_env)?;
    assert!(!second_output.status().success());
    let stderr = second_output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn unlisted_host_fails() -> Result<()> {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = OTHERHOSTS, !SERVERS",
        "ALL WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("not_listed")
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn negation_not_order_sensitive() -> Result<()> {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = !SERVERS, OTHERHOSTS",
        "ALL WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[ignore = "gh379"]
#[test]
fn negation_combination() -> Result<()> {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = !SERVERS, OTHERHOSTS",
        "ALL !WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(output.status().success());

    Ok(())
}

#[test]
fn comma_listing_works() -> Result<()> {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = OTHERHOSTS",
        "ALL SERVERS, WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("foo")
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(output.status().success());
    let second_env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = OTHERHOSTS",
        "ALL SERVERS, WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build()?;

    let second_output = Command::new("sudo").arg("true").output(&second_env)?;

    assert!(second_output.status().success());

    Ok(())
}
