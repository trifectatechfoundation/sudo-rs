use sudo_test::{Command, Env, BIN_LS, BIN_TRUE};

use crate::{Result, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![(r"[[:xdigit:]]{12}", "[host]")],
            prepend_module_to_snapshot => false,
            snapshot_path => "../../snapshots/sudoers/cmnd_alias",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn cmnd_alias_works() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias CMDSGROUP = {BIN_TRUE}, {BIN_LS}"),
        "ALL ALL=(ALL:ALL) CMDSGROUP".to_owned(),
    ])
    .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn cmnd_alias_nopasswd() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias CMDSGROUP = {BIN_TRUE}, {BIN_LS}"),
        "ALL ALL=(ALL:ALL) NOPASSWD: CMDSGROUP".to_owned(),
    ])
    .user(USERNAME)
    .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()
}

#[test]
fn cmnd_alias_can_contain_underscore_and_digits() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias UNDER_SCORE123 = {BIN_TRUE}, {BIN_LS}"),
        "ALL ALL=(ALL:ALL) UNDER_SCORE123".to_owned(),
    ])
    .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn cmnd_alias_cannot_start_with_underscore() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias _INVALID = {BIN_TRUE}"),
        "ALL ALL=(ALL:ALL) NOPASSWD: ALL".to_owned(),
        "ALL ALL=(ALL:ALL) !_INVALID".to_owned(),
    ])
    .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn unlisted_cmnd_fails() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias CMDS = {BIN_LS}"),
        "ALL ALL=(ALL:ALL) CMDSGROUP".to_owned(),
    ])
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn command_specified_not_by_absolute_path_is_rejected() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias CMDSGROUP = true, {BIN_LS}"),
        "ALL ALL=(ALL:ALL) CMDSGROUP".to_owned(),
    ])
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn command_alias_negation() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias CMDSGROUP = {BIN_TRUE}, {BIN_LS}"),
        "ALL ALL=(ALL:ALL) !CMDSGROUP".to_owned(),
    ])
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn combined_cmnd_aliases() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias TRUEGROUP = /usr/bin/sh, {BIN_TRUE}"),
        format!("Cmnd_Alias LSGROUP = {BIN_LS}, /usr/sbin/dump"),
        "Cmnd_Alias BAZ = !TRUEGROUP, LSGROUP".to_owned(),
        "ALL ALL=(ALL:ALL) BAZ".to_owned(),
    ])
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());
    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "I'm sorry root. I'm afraid I can't do that"
        );
    }

    let second_output = Command::new("sudo").arg("ls").output(&env)?;

    assert!(second_output.status().success());

    Ok(())
}

#[test]
fn double_negation() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias CMDSGROUP = {BIN_TRUE}, {BIN_LS}"),
        "ALL ALL=(ALL:ALL) !!CMDSGROUP".to_owned(),
    ])
    .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()
}

#[test]
fn negation_not_order_sensitive() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias TRUECMND = {BIN_TRUE}"),
        format!("Cmnd_Alias LSCMND = {BIN_LS}"),
        "Cmnd_Alias BAZ = TRUECMND, !LSCMND".to_owned(),
        "ALL ALL=(ALL:ALL) BAZ".to_owned(),
    ])
    .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo").arg("ls").output(&env)?;
    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn negation_combination() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias TRUECMND = !{BIN_TRUE}"),
        format!("Cmnd_Alias LSCMND = {BIN_LS}"),
        "Cmnd_Alias BAZ = !TRUECMND, LSCMND".to_owned(),
        "ALL ALL=(ALL:ALL) BAZ".to_owned(),
    ])
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(output.status().success());

    let second_output = Command::new("sudo").arg("ls").output(&env)?;

    assert!(second_output.status().success());

    Ok(())
}

#[test]
fn another_negation_combination() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias TRUECMND = {BIN_TRUE}"),
        format!("Cmnd_Alias LSCMND = {BIN_LS}"),
        "Cmnd_Alias BAZ = TRUECMND, !LSCMND".to_owned(),
        "ALL ALL=(ALL:ALL) !BAZ".to_owned(),
    ])
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "I'm sorry root. I'm afraid I can't do that"
        );
    }

    let second_output = Command::new("sudo").arg("ls").output(&env)?;

    assert!(second_output.status().success());

    Ok(())
}

#[test]
fn one_more_negation_combination() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias TRUECMND = {BIN_TRUE}"),
        format!("Cmnd_Alias LSCMND = !{BIN_LS}"),
        "Cmnd_Alias BAZ = TRUECMND, LSCMND".to_owned(),
        "ALL ALL=(ALL:ALL) !BAZ".to_owned(),
    ])
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "I'm sorry root. I'm afraid I can't do that"
        );
    }

    let second_output = Command::new("sudo").arg("ls").output(&env)?;

    assert!(second_output.status().success());

    Ok(())
}

#[test]
fn tripple_negation_combination() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias TRUECMND = {BIN_TRUE}"),
        format!("Cmnd_Alias LSCMND = !{BIN_LS}"),
        "Cmnd_Alias BAZ = TRUECMND, !LSCMND".to_owned(),
        "ALL ALL=(ALL:ALL) !BAZ".to_owned(),
    ])
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "I'm sorry root. I'm afraid I can't do that"
        );
    }

    let second_output = Command::new("sudo").arg("ls").output(&env)?;

    assert!(!second_output.status().success());

    let stderr = second_output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn comma_listing_works() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias TRUEGROUP = /usr/bin/sh, {BIN_TRUE}"),
        format!("Cmnd_Alias LSGROUP = {BIN_LS}, /usr/sbin/dump"),
        "ALL ALL=(ALL:ALL) TRUEGROUP, LSGROUP".to_owned(),
    ])
    .build()?;

    let output = Command::new("sudo").arg("true").output(&env)?;

    assert!(output.status().success());

    let second_output = Command::new("sudo").arg("ls").output(&env)?;

    assert!(second_output.status().success());

    Ok(())
}

#[test]
fn runas_override() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias TRUECMND = {BIN_TRUE}"),
        format!("Cmnd_Alias LSCMND = {BIN_LS}"),
        "ALL ALL = (root) LSCMND, (ferris) TRUECMND".to_owned(),
    ])
    .user("ferris")
    .build()?;

    let output = Command::new("sudo").args([BIN_LS, "/root"]).output(&env)?;
    assert!(output.status().success());

    let output = Command::new("sudo")
        .args(["-u", "ferris", BIN_LS])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "I'm sorry root. I'm afraid I can't do that"
        );
    }

    Command::new("sudo")
        .args(["-u", "ferris", BIN_TRUE])
        .output(&env)?
        .assert_success()?;

    let second_output = Command::new("sudo").arg(BIN_TRUE).output(&env)?;

    assert!(!second_output.status().success());
    assert_eq!(Some(1), second_output.status().code());

    let stderr = second_output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn runas_override_repeated_cmnd_means_runas_union() -> Result<()> {
    let env = Env([
        format!("Cmnd_Alias TRUECMND = {BIN_TRUE}"),
        format!("Cmnd_Alias LSCMND = {BIN_LS}"),
        "ALL ALL = (root) TRUECMND, (ferris) TRUECMND".to_owned(),
    ])
    .user("ferris")
    .build()?;

    Command::new("sudo")
        .arg("true")
        .output(&env)?
        .assert_success()?;

    Command::new("sudo")
        .args(["-u", "ferris", "true"])
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
#[ignore = "gh700"]
fn keywords() -> Result<()> {
    for bad_keyword in super::KEYWORDS_ALIAS_BAD {
        dbg!(bad_keyword);
        let env = Env([
            format!("Cmnd_Alias {bad_keyword} = {BIN_TRUE}"),
            format!("ALL ALL=(ALL:ALL) {bad_keyword}"),
        ])
        .build()?;

        let output = Command::new("sudo").arg("true").output(&env)?;

        assert_contains!(output.stderr(), "syntax error");
        assert_eq!(*bad_keyword == "ALL", output.status().success());
    }

    for good_keyword in super::keywords_alias_good() {
        dbg!(good_keyword);
        let env = Env([
            format!("Cmnd_Alias {good_keyword} = {BIN_TRUE}"),
            format!("ALL ALL=(ALL:ALL) {good_keyword}"),
        ])
        .build()?;

        let output = Command::new("sudo").arg("true").output(&env)?;

        let stderr = output.stderr();
        assert!(stderr.is_empty(), "{}", stderr);
        assert!(output.status().success());
    }

    Ok(())
}
