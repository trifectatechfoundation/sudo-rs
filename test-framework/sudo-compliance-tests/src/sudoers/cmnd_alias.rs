use sudo_test::{Command, Env};

use crate::{Result, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![(r"[[:xdigit:]]{12}", "[host]")],
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/sudoers/cmnd_alias",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn cmnd_alias_works() -> Result<()> {
    let env = Env([
        "Cmnd_Alias CMDSGROUP = /bin/true, /bin/ls",
        "ALL ALL=(ALL:ALL) CMDSGROUP",
    ])
    .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}

#[test]
fn cmnd_alias_nopasswd() -> Result<()> {
    let env = Env([
        "Cmnd_Alias CMDSGROUP = /bin/true, /bin/ls",
        "ALL ALL=(ALL:ALL) NOPASSWD: CMDSGROUP",
    ])
    .user(USERNAME)
    .build()?;

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}

#[test]
fn cmnd_alias_underscore() -> Result<()> {
    let env = Env([
        "Cmnd_Alias UNDER_SCORE = /bin/true, /bin/ls",
        "ALL ALL=(ALL:ALL) UNDER_SCORE",
    ])
    .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()
}

#[test]
fn unlisted_cmnd_fails() -> Result<()> {
    let env = Env([
        "Cmnd_Alias CMDS = /bin/ls",
        "ALL ALL=(ALL:ALL) CMDSGROUP",
    ])
    .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .exec(&env)?;

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
        "Cmnd_Alias CMDSGROUP = true, /bin/ls",
        "ALL ALL=(ALL:ALL) CMDSGROUP",
    ])
    .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .exec(&env)?;

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
        "Cmnd_Alias CMDSGROUP = /bin/true, /bin/ls",
        "ALL ALL=(ALL:ALL) !CMDSGROUP",
        ])
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .exec(&env)?;

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
        "Cmnd_Alias TRUEGROUP = /usr/bin/sh, /bin/true",
        "Cmnd_Alias LSGROUP = /bin/ls, /usr/sbin/dump",
        "Cmnd_Alias BAZ = !TRUEGROUP, LSGROUP",
        "ALL ALL=(ALL:ALL) BAZ",
    ])
    .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .exec(&env)?;

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

    let second_output = Command::new("sudo")
    .arg("ls")
    .exec(&env)?;

    assert!(second_output.status().success());

    Ok(())
}

#[test]
fn double_negation() -> Result<()> {
    let env = Env([
        "Cmnd_Alias CMDSGROUP = /bin/true, /bin/ls",
        "ALL ALL=(ALL:ALL) !!CMDSGROUP",
        ])
        .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()

}

#[test]
fn negation_not_order_sensitive() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUECMND = /bin/true",
        "Cmnd_Alias LSCMND = /bin/ls",
        "Cmnd_Alias BAZ = TRUECMND, !LSCMND",
        "ALL ALL=(ALL:ALL) BAZ",
        ])
        .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .arg("ls")
        .exec(&env)?;
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

#[ignore]
#[test]
fn negation_combination() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUECMND = !/bin/true",
        "Cmnd_Alias LSCMND = /bin/ls",
        "Cmnd_Alias BAZ = !TRUECMND, LSCMND",
        "ALL ALL=(ALL:ALL) BAZ",
        ])
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .exec(&env)?;

        assert!(output.status().success());

        let second_output = Command::new("sudo")
        .arg("ls")
        .exec(&env)?;
    
        assert!(second_output.status().success());

        Ok(())
}

#[ignore]
#[test]
fn another_negation_combination() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUECMND = /bin/true",
        "Cmnd_Alias LSCMND = /bin/ls",
        "Cmnd_Alias BAZ = TRUECMND, !LSCMND",
        "ALL ALL=(ALL:ALL) !BAZ",
        ])
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .exec(&env)?;

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

        let second_output = Command::new("sudo")
        .arg("ls")
        .exec(&env)?;
    
        assert!(second_output.status().success());

        Ok(())
}

#[ignore]
#[test]
fn one_more_negation_combination() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUECMND = /bin/true",
        "Cmnd_Alias LSCMND = !/bin/ls",
        "Cmnd_Alias BAZ = TRUECMND, LSCMND",
        "ALL ALL=(ALL:ALL) !BAZ",
        ])
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .exec(&env)?;

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

        let second_output = Command::new("sudo")
        .arg("ls")
        .exec(&env)?;
    
        assert!(second_output.status().success());

        Ok(())
}

#[test]
fn tripple_negation_combination() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUECMND = /bin/true",
        "Cmnd_Alias LSCMND = !/bin/ls",
        "Cmnd_Alias BAZ = TRUECMND, !LSCMND",
        "ALL ALL=(ALL:ALL) !BAZ",
        ])
        .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .exec(&env)?;

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

        let second_output = Command::new("sudo")
        .arg("ls")
        .exec(&env)?;
    
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
        "Cmnd_Alias TRUEGROUP = /usr/bin/sh, /bin/true",
        "Cmnd_Alias LSGROUP = /bin/ls, /usr/sbin/dump",
        "ALL ALL=(ALL:ALL) TRUEGROUP, LSGROUP",
    ])
    .build()?;

    let output = Command::new("sudo")
        .arg("true")
        .exec(&env)?;

    assert!(output.status().success());

    let second_output = Command::new("sudo")
    .arg("ls")
    .exec(&env)?;

    assert!(second_output.status().success());

    Ok(())

}

#[test]
fn runas_override() -> Result<()> {
    let env = Env([
        "Cmnd_Alias TRUECMND = /bin/true",
        "Cmnd_Alias LSCMND = /bin/ls",
        "ALL ALL = (root) LSCMND, (ferris) TRUECMND",
    ])
    .user("ferris")
    .build()?;

    let stdout = Command::new("sudo")
        .args(["/bin/ls", "/root"])
        .exec(&env)?
        .stdout()?;
    assert_eq!("", stdout);

    let output = Command::new("sudo")
        .args(["-u", "ferris", "/bin/ls"])
        .exec(&env)?;

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
        .args(["-u", "ferris", "/bin/true"])
        .exec(&env)?
        .assert_success()?;

    let second_output = Command::new("sudo").args(["/bin/true"]).exec(&env)?;

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
        "Cmnd_Alias TRUECMND = /bin/true",
        "Cmnd_Alias LSCMND = /bin/ls",
        "ALL ALL = (root) TRUECMND, (ferris) TRUECMND",
    ])
    .user("ferris")
    .build()?;

    Command::new("sudo")
        .arg("true")
        .exec(&env)?
        .assert_success()?;

    Command::new("sudo")
        .args(["-u", "ferris", "true"])
        .exec(&env)?
        .assert_success()?;

    Ok(())
}
