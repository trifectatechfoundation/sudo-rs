use sudo_test::{Command, Env, User};

use crate::{Result, SUDOERS_NO_LECTURE, USERNAME, PASSWORD};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![(r"[[:xdigit:]]{12}", "[host]")],
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/sudoers/runas_alias",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn runas_alias_works() -> Result<()> {
    let env = Env([
        "Runas_Alias OP = root, operator",
        "root ALL=(ALL:ALL) NOPASSWD: ALL",
        "ferris ALL = (OP) ALL",
    ])
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-u", "root", "-S", "true"])
            .as_user( user)
            .stdin(PASSWORD)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn underscore() -> Result<()> {
    let env = Env([
        "Runas_Alias UNDER_SCORE = root, operator",
        "root ALL=(ALL:ALL) NOPASSWD: ALL",
        "ferris ALL = (UNDER_SCORE) ALL",
    ])
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-u", "root", "-S", "true"])
            .as_user( user)
            .stdin(PASSWORD)
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn runas_alias_negation() -> Result<()> {
        let env = Env([
            "Runas_Alias OP = root, operator",
            "root ALL = (ALL:ALL) NOPASSWD: ALL",
            "ferris ALL = (!OP) ALL",
            SUDOERS_NO_LECTURE
        ])
            .user(User("ferris").password(PASSWORD))
            .build()?;
    
        let output = Command::new("sudo")
            .args(["-u", "root", "-S", "true"])
            .as_user("ferris")
            .stdin(PASSWORD)
            .exec(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());
        let stderr = output.stderr();
        if sudo_test::is_original_sudo() {
            assert_snapshot!(stderr);
        } else {
            assert_contains!(
                stderr,
                format!("authentication failed: I'm sorry ferris. I'm afraid I can't do that")
            );
        }
    
        Ok(())
    }

    #[test]
    fn negation_on_user() -> Result<()> {
            let env = Env([
                "Runas_Alias OP = !root, operator",
                "root ALL = (ALL:ALL) NOPASSWD: ALL",
                "ferris ALL = (OP) ALL",
                SUDOERS_NO_LECTURE
            ])
                .user(User("ferris").password(PASSWORD))
                .build()?;
        
            let output = Command::new("sudo")
                .args(["-u", "root", "-S", "true"])
                .as_user("ferris")
                .stdin(PASSWORD)
                .exec(&env)?;
    
            assert!(!output.status().success());
            assert_eq!(Some(1), output.status().code());
            let stderr = output.stderr();
            if sudo_test::is_original_sudo() {
                assert_snapshot!(stderr);
            } else {
                assert_contains!(
                    stderr,
                    format!("authentication failed: I'm sorry ferris. I'm afraid I can't do that")
                );
            }
        
            Ok(())
        }

    #[test]
    fn double_negation() -> Result<()> {
        let env = Env([
            "Runas_Alias OP = root, operator",
            "root ALL=(ALL:ALL) NOPASSWD: ALL",
            "ferris ALL = (!!OP) ALL",
        ])
            .user(User(USERNAME).password(PASSWORD))
            .build()?;
    
        for user in ["root", USERNAME] {
            Command::new("sudo")
                .args(["-u", "root", "-S", "true"])
                .as_user( user)
                .stdin(PASSWORD)
                .exec(&env)?
                .assert_success()?;
        }
    
        Ok(())
    }

    #[test]
    fn when_specific_user_then_as_a_different_user_is_not_allowed() -> Result<()> {
        let env = Env([
            "Runas_Alias OP = ferris, operator",
            "ALL ALL = (OP) ALL",
            SUDOERS_NO_LECTURE
        ])
            .user(User("ferris").password(PASSWORD))
            .user(User("ghost").password(PASSWORD))
            .build()?;

        let output = Command::new("sudo")
            .args(["-u", "ghost", "-S", "true"])
            .as_user( "ferris")
            .stdin(PASSWORD)
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
