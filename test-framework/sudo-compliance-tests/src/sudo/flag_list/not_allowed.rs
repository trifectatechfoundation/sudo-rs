use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

#[test]
fn flag_uppercase_u() {
    let other_user = "ghost";
    let hostname = "container";

    let sudoerss = &[
        String::new(),
        format!("{USERNAME} ALL=(ALL:ALL) /dev/null"),
        format!("{other_user} ALL=(ALL:ALL) /dev/null"),
        "ALL ALL=(ALL:ALL) /dev/null".to_string(),
    ];

    for sudoers in sudoerss {
        dbg!(sudoers);

        let env = Env(sudoers.as_str())
            .user(User(USERNAME).password(PASSWORD))
            .user(other_user)
            .hostname(hostname)
            .build();

        let output = Command::new("sudo")
            .args(["-S", "-U", other_user, "-l"])
            .as_user(USERNAME)
            .stdin(PASSWORD)
            .output(&env);

        output.assert_exit_code(1);

        let diagnostic = format!(
            "Sorry, user {USERNAME} is not allowed to execute 'list' as {other_user} on {hostname}."
        );
        assert_contains!(output.stderr(), diagnostic);
    }
}

#[test]
fn flag_uppercase_u_plus_command() {
    let other_user = "ghost";
    let hostname = "container";

    let sudoerss = &[
        String::new(),
        format!("{USERNAME} ALL=(ALL:ALL) /dev/null"),
        format!("{other_user} ALL=(ALL:ALL) /dev/null"),
        "ALL ALL=(ALL:ALL) /dev/null".to_string(),
    ];

    for sudoers in sudoerss {
        let env = Env(sudoers.as_str())
            .user(User(USERNAME).password(PASSWORD))
            .user(other_user)
            .hostname(hostname)
            .build();

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
                .output(&env);

            output.assert_exit_code(1);

            // This is the output of older sudo versions
            if !output.stderr().contains(&format!(
                "Sorry, user {USERNAME} is not allowed to execute 'list/usr/bin/true' \
                 as {other_user} on {hostname}."
            )) {
                // This is the output of newer sudo versions and sudo-rs
                let diagnostic = format!(
                    "Sorry, user {USERNAME} is not allowed to execute 'list true' as {other_user} on {hostname}."
                );
                assert_contains!(output.stderr(), diagnostic);
            }
        }
    }
}

#[test]
fn other_cases() {
    let other_user = "ghost";
    let hostname = "container";
    let env = Env("")
        .user(User(USERNAME).password(PASSWORD))
        .user(other_user)
        .hostname(hostname)
        .build();

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
            .output(&env);

        output.assert_exit_code(1);

        let diagnostic = format!("Sorry, user {USERNAME} may not run sudo on {hostname}.");
        assert_contains!(output.stderr(), diagnostic);
    }
}
