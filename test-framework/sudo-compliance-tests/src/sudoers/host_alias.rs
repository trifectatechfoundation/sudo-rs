use sudo_test::{Command, Env};

use crate::{Result};

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
        .args(["-h", "mail", "true"])
        .exec(&env)?
        .assert_success()

}

#[test]
fn host_alias_negation() -> Result<()> {
    let env = Env([
            "Host_Alias SERVERS = main, www, mail",
            "ALL !SERVERS=(ALL:ALL) ALL",
        ])
        .hostname("mail")
        .build()?;

    let output = Command::new("sudo")
        .args(["-h", "mail", "true"])
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
fn host_alias_double_negation() -> Result<()> {
    let env = Env([
            "Host_Alias SERVERS = main, www, mail",
            "ALL !!SERVERS=(ALL:ALL) ALL",
        ])
        .hostname("mail")
        .build()?;

    Command::new("sudo")
        .args(["-h", "mail", "true"])
        .exec(&env)?
        .assert_success()

}

#[ignore]
#[test]
fn combined_host_aliases() -> Result<()> {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = OTHERHOSTS, !SERVERS",
        "ALL WORKSTATIONS=(ALL:ALL) ALL",
        ])
        .hostname("foo")
        .hostname("mail")
        .build()?;

    let second_output = Command::new("sudo")
        .args(["-h", "foo", "true"])
        .exec(&env)?;
        assert!(second_output.status().success());

    let output = Command::new("sudo")
        .args(["-h", "mail", "true"])
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
