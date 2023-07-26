use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

#[test]
fn flag_uppercase_u() -> Result<()> {
    let other_user = "ghost";
    let hostname = "container";

    let sudoerss = &[String::new(), format!("{USERNAME} ALL=(ALL:ALL) /dev/null")];

    for sudoers in sudoerss {
        dbg!(sudoers);

        let env = Env(sudoers.as_str())
            .user(User(USERNAME).password(PASSWORD))
            .user(other_user)
            .hostname(hostname)
            .build()?;

        let output = Command::new("sudo")
            .args(["-S", "-U", other_user, "-l"])
            .as_user(USERNAME)
            .stdin(PASSWORD)
            .output(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        let diagnostic = format!(
        "Sorry, user {USERNAME} is not allowed to execute 'list' as {other_user} on {hostname}."
    );
        assert_contains!(output.stderr(), diagnostic);
    }

    Ok(())
}

#[test]
fn flag_uppercase_u_plus_command() -> Result<()> {
    let other_user = "ghost";
    let hostname = "container";

    let sudoerss = &[String::new(), format!("{USERNAME} ALL=(ALL:ALL) /dev/null")];

    for sudoers in sudoerss {
        let env = Env(sudoers.as_str())
            .user(User(USERNAME).password(PASSWORD))
            .user(other_user)
            .hostname(hostname)
            .build()?;

        // `-u` has no effect on the diagnostic
        let argss: &[&[&str]] = &[
            &["-S", "-U", other_user, "-l", "true"],
            &["-S", "-U", other_user, "-u", other_user, "-l", "true"],
        ];

        for args in argss {
            dbg!(sudoers, args);

            let output = Command::new("sudo")
                .args(*args)
                .as_user(USERNAME)
                .stdin(PASSWORD)
                .output(&env)?;

            assert!(!output.status().success());
            assert_eq!(Some(1), output.status().code());

            let command = if sudo_test::is_original_sudo() {
                "list/usr/bin/true"
            } else {
                "list true"
            };
            let diagnostic =
        format!("Sorry, user {USERNAME} is not allowed to execute '{command}' as {other_user} on {hostname}.");
            assert_contains!(output.stderr(), diagnostic);
        }
    }

    Ok(())
}

#[test]
fn other_cases() -> Result<()> {
    let other_user = "ghost";
    let hostname = "container";
    let env = Env("")
        .user(User(USERNAME).password(PASSWORD))
        .user(other_user)
        .hostname(hostname)
        .build()?;

    let argss: &[&[&str]] = &[
        &["-S", "-l"],
        &["-S", "-l", "true"],
        &["-S", "-u", other_user, "-l", "true"],
    ];

    for args in argss {
        dbg!(args);

        let output = Command::new("sudo")
            .args(*args)
            .as_user(USERNAME)
            .stdin(PASSWORD)
            .output(&env)?;

        assert!(!output.status().success());
        assert_eq!(Some(1), output.status().code());

        let diagnostic = format!("Sorry, user {USERNAME} may not run sudo on {hostname}.");
        assert_contains!(output.stderr(), diagnostic);
    }

    Ok(())
}
