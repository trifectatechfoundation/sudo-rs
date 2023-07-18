use sudo_test::{Command, Env};

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
        "Cmnd_Alias CMDSGROUP = /usr/bin/true, /usr/bin/ls",
        "ALL ALL=(ALL:ALL) CMDSGROUP",
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
        "Cmnd_Alias CMDSGROUP = /usr/bin/true, /usr/bin/ls",
        "ALL ALL=(ALL:ALL) NOPASSWD: CMDSGROUP",
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
        "Cmnd_Alias UNDER_SCORE123 = /usr/bin/true, /usr/bin/ls",
        "ALL ALL=(ALL:ALL) UNDER_SCORE123",
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
        "Cmnd_Alias _INVALID = /usr/bin/true",
        "ALL ALL=(ALL:ALL) NOPASSWD: ALL",
        "ALL ALL=(ALL:ALL) !_INVALID",
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
        "Cmnd_Alias CMDS = /usr/bin/ls",
        "ALL ALL=(ALL:ALL) CMDSGROUP",
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
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn command_specified_not_by_absolute_path_is_rejected() -> Result<()> {
    let env = Env([
        "Cmnd_Alias CMDSGROUP = true, /usr/bin/ls",
        "ALL ALL=(ALL:ALL) CMDSGROUP",
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
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn command_alias_negation() -> Result<()> {
    let env = Env([
        "Cmnd_Alias CMDSGROUP = /usr/bin/true, /usr/bin/ls",
        "ALL ALL=(ALL:ALL) !CMDSGROUP",
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
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn combined_cmnd_aliases() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUEGROUP = /usr/bin/sh, /usr/bin/true",
        "Cmnd_Alias LSGROUP = /usr/bin/ls, /usr/sbin/dump",
        "Cmnd_Alias BAZ = !TRUEGROUP, LSGROUP",
        "ALL ALL=(ALL:ALL) BAZ",
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
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    let second_output = Command::new("sudo").arg("ls").output(&env)?;

    assert!(second_output.status().success());

    Ok(())
}

#[test]
fn double_negation() -> Result<()> {
    let env = Env([
        "Cmnd_Alias CMDSGROUP = /usr/bin/true, /usr/bin/ls",
        "ALL ALL=(ALL:ALL) !!CMDSGROUP",
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
        "Cmnd_Alias TRUECMND = /usr/bin/true",
        "Cmnd_Alias LSCMND = /usr/bin/ls",
        "Cmnd_Alias BAZ = TRUECMND, !LSCMND",
        "ALL ALL=(ALL:ALL) BAZ",
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
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn negation_combination() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUECMND = !/usr/bin/true",
        "Cmnd_Alias LSCMND = /usr/bin/ls",
        "Cmnd_Alias BAZ = !TRUECMND, LSCMND",
        "ALL ALL=(ALL:ALL) BAZ",
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
        "Cmnd_Alias TRUECMND = /usr/bin/true",
        "Cmnd_Alias LSCMND = /usr/bin/ls",
        "Cmnd_Alias BAZ = TRUECMND, !LSCMND",
        "ALL ALL=(ALL:ALL) !BAZ",
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
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    let second_output = Command::new("sudo").arg("ls").output(&env)?;

    assert!(second_output.status().success());

    Ok(())
}

#[test]
fn one_more_negation_combination() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUECMND = /usr/bin/true",
        "Cmnd_Alias LSCMND = !/usr/bin/ls",
        "Cmnd_Alias BAZ = TRUECMND, LSCMND",
        "ALL ALL=(ALL:ALL) !BAZ",
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
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    let second_output = Command::new("sudo").arg("ls").output(&env)?;

    assert!(second_output.status().success());

    Ok(())
}

#[test]
fn tripple_negation_combination() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUECMND = /usr/bin/true",
        "Cmnd_Alias LSCMND = !/usr/bin/ls",
        "Cmnd_Alias BAZ = TRUECMND, !LSCMND",
        "ALL ALL=(ALL:ALL) !BAZ",
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
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
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
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    Ok(())
}

#[test]
fn comma_listing_works() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUEGROUP = /usr/bin/sh, /usr/bin/true",
        "Cmnd_Alias LSGROUP = /usr/bin/ls, /usr/sbin/dump",
        "ALL ALL=(ALL:ALL) TRUEGROUP, LSGROUP",
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
        "Cmnd_Alias TRUECMND = /usr/bin/true",
        "Cmnd_Alias LSCMND = /usr/bin/ls",
        "ALL ALL = (root) LSCMND, (ferris) TRUECMND",
    ])
    .user("ferris")
    .build()?;

    let stdout = Command::new("sudo")
        .args(["/usr/bin/ls", "/root"])
        .output(&env)?
        .stdout()?;
    assert_eq!("", stdout);

    let output = Command::new("sudo")
        .args(["-u", "ferris", "/usr/bin/ls"])
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            "authentication failed: I'm sorry root. I'm afraid I can't do that"
        );
    }

    Command::new("sudo")
        .args(["-u", "ferris", "/usr/bin/true"])
        .output(&env)?
        .assert_success()?;

    let second_output = Command::new("sudo").args(["/usr/bin/true"]).output(&env)?;

    assert!(!second_output.status().success());
    assert_eq!(Some(1), second_output.status().code());

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
fn runas_override_repeated_cmnd_means_runas_union() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUECMND = /usr/bin/true",
        "Cmnd_Alias LSCMND = /usr/bin/ls",
        "ALL ALL = (root) TRUECMND, (ferris) TRUECMND",
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
            format!("Cmnd_Alias {bad_keyword} = /usr/bin/true"),
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
            format!("Cmnd_Alias {good_keyword} = /usr/bin/true"),
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
