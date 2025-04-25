use sudo_test::{Command, Env, TextFile, BIN_SUDO};

use crate::SUDOERS_ALL_ALL_NOPASSWD;

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../../snapshots/sudoers/secure_path",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn if_unset_searches_program_in_invoking_users_path() {
    let path = "/root/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(path, TextFile("#!/bin/sh").chmod("100"))
        .build();

    Command::new("sh")
        .args([
            "-c",
            &format!("export PATH=/root; cd /; {BIN_SUDO} my-script"),
        ])
        .output(&env)
        .assert_success();
}

#[test]
fn if_set_searches_program_in_secure_path() {
    let path = "/root/my-script";
    let env = Env("\
Defaults secure_path=.:/root
ALL ALL=(ALL:ALL) NOPASSWD: ALL")
    .file(path, TextFile("#!/bin/sh").chmod("100"))
    .build();

    // `true` is in `/usr/bin/`
    let match_in_relative_path_when_path_is_unset =
        format!("unset PATH; cd /usr/bin; {BIN_SUDO} true");
    let match_in_absolute_path_when_path_is_unset =
        format!("unset PATH; cd /; {BIN_SUDO} my-script");
    let match_in_relative_path_when_path_is_set =
        format!("export PATH=/tmp; cd /usr/bin; {BIN_SUDO} true");
    let match_in_absolute_path_when_path_is_set =
        format!("export PATH=/tmp; cd /; {BIN_SUDO} my-script");

    let scripts = [
        match_in_relative_path_when_path_is_unset,
        match_in_absolute_path_when_path_is_unset,
        match_in_relative_path_when_path_is_set,
        match_in_absolute_path_when_path_is_set,
    ];

    for script in scripts {
        println!("{script}");

        Command::new("sh")
            .args(["-c", &script])
            .output(&env)
            .assert_success();
    }
}

#[test]
fn if_set_it_does_not_search_in_original_user_path() {
    let env = Env("\
        Defaults secure_path=/root
ALL ALL=(ALL:ALL) NOPASSWD: ALL")
    .build();

    let output = Command::new("sudo").arg("true").output(&env);

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "'true': command not found");
    }
}

#[test]
fn if_set_it_becomes_the_path_set_for_program_execution() {
    let secure_path = ".:/root";
    let env = Env(format!(
        "Defaults secure_path={secure_path}
ALL ALL=(ALL:ALL) NOPASSWD: ALL"
    ))
    .build();

    let user_path_set = "cd /; sudo /usr/bin/printenv PATH";
    let user_path_unset = format!("unset PATH; cd /; {BIN_SUDO} /usr/bin/printenv PATH");
    let scripts = [user_path_set, &user_path_unset];

    for script in scripts {
        println!("{script}");

        let path = Command::new("sh")
            .args(["-c", script])
            .output(&env)
            .stdout();

        assert_eq!(secure_path, &path);
    }
}
