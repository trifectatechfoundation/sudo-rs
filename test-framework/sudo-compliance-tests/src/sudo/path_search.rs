use sudo_test::{BIN_SUDO, Command, Env, TextFile};

use crate::{SUDOERS_ALL_ALL_NOPASSWD, USERNAME, helpers};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/path_search",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn can_find_command_not_visible_to_regular_user() {
    let path = "/root/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(path, TextFile("#!/bin/sh").chmod("100"))
        .build();

    Command::new("sh")
        .args([
            "-c",
            &format!("export PATH=/root; cd /; {BIN_SUDO} my-script"),
        ])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
//Cross-reference: sudoers::secure_path::if_set_searches_program_in_secure_path for
//testing that relative paths in secure_path are also not matched.
fn does_not_use_relative_paths() {
    let path = "/root/my-script";
    let env = Env("Defaults ignore_dot
ALL ALL=(ALL:ALL) NOPASSWD: ALL")
    .user(USERNAME)
    .file(path, TextFile("#!/bin/sh").chmod("100"))
    .build();

    let output = Command::new("sh")
        .args([
            "-c",
            &format!("export PATH=.; cd /root; {BIN_SUDO} my-script"),
        ])
        .output(&env);

    output.assert_exit_code(1);

    if sudo_test::is_original_sudo() {
        assert_eq!(
            output.stderr(),
            "sudo: ignoring \"my-script\" found in '.'
Use \"sudo ./my-script\" if this is the \"my-script\" you wish to run."
        );
    } else {
        //NOTE: we don't have a specialized error message for this case
        assert_eq!(output.stderr(), "sudo: 'my-script': command not found");
    }
}

#[test]
fn when_path_is_unset_does_not_search_in_default_path_set_for_command_execution() {
    let path = "/usr/bin/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(path, TextFile("#!/bin/sh").chmod("777"))
        .build();

    let default_path = Command::new("sh")
        .args([
            "-c",
            &format!("unset PATH; {BIN_SUDO} /usr/bin/printenv PATH"),
        ])
        .output(&env)
        .stdout();

    // sanity check that `/usr/bin` is in sudo's default PATH
    let default_path = helpers::parse_path(&default_path);
    assert!(default_path.contains("/usr/bin"));

    let output = Command::new("sh")
        .args(["-c", &format!("unset PATH; {BIN_SUDO} my-script")])
        .output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "'my-script': command not found");
    }
}

#[test]
fn ignores_path_for_qualified_commands() {
    let path = "/root/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(path, TextFile("#!/bin/sh").chmod("100"))
        .build();

    for param in ["/root/my-script", "./my-script"] {
        Command::new("sh")
            .args(["-c", &format!("cd /root; sudo {param}")])
            .as_user("root")
            .output(&env)
            .assert_success();
    }
}

#[test]
fn paths_are_matched_using_realpath_in_sudoers() {
    let env = Env(["ALL ALL = /tmp/bin/true"]).build();

    Command::new("ln")
        .args(["-s", "/usr/bin", "/tmp/bin"])
        .output(&env)
        .assert_success();

    Command::new("sudo")
        .arg("/usr/bin/true")
        .output(&env)
        .assert_success();
}

#[test]
fn paths_are_matched_using_realpath_in_arguments() {
    let env = Env(["ALL ALL = /usr/bin/true"]).build();

    Command::new("ln")
        .args(["-s", "/usr/bin", "/tmp/bin"])
        .output(&env)
        .assert_success();

    Command::new("sudo")
        .arg("/tmp/bin/true")
        .output(&env)
        .assert_success();
}

#[test]
fn arg0_native_is_passed_from_commandline() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build();

    // On FreeBSD awk is one of the few programs which print arg0 in error messages. On Linux
    // however it doesn't print arg0 unlike most programs, so we use a random program instead.
    if cfg!(target_os = "freebsd") {
        let output = Command::new("sh")
            .args([
                "-c",
                "ln -s /usr/bin /nib; sudo /nib/awk --invalid-flag; true",
            ])
            .output(&env);

        let stderr = output.stderr();
        assert_starts_with!(stderr, "/nib/awk:");
    } else {
        let output = Command::new("sh")
            .args(["-c", "ln -s /bin /nib; sudo /nib/ls --invalid-flag; true"])
            .output(&env);

        let stderr = output.stderr();
        assert_starts_with!(stderr, "/nib/ls:");
    }
}

#[test]
fn arg0_native_is_resolved_from_commandline() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build();

    let output = Command::new("sh")
        .args([
            "-c",
            "ln -s /bin/ls /bin/foo; sudo foo --invalid-flag; true",
        ])
        .output(&env);

    let stderr = output.stderr();
    assert_starts_with!(stderr, "foo: unrecognized option");
}

#[test]
#[ignore = "gh735"]
fn arg0_script_is_passed_from_commandline() {
    let path = "/bin/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(path, TextFile("#!/bin/sh\necho $0").chmod("777"))
        .build();

    let output = Command::new("sh")
        .args(["-c", "ln -s /bin /nib; sudo /nib/my-script"])
        .output(&env);

    let stdout = output.stdout();
    assert_eq!(stdout, "/nib/my-script");
}

#[test]
fn arg0_script_is_resolved_from_commandline() {
    let path = "/bin/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(path, TextFile("#!/bin/sh\necho $0").chmod("777"))
        .build();

    let output = Command::new("sh")
        .args(["-c", &format!("ln -s {path} /usr/bin/foo; sudo foo")])
        .output(&env);

    let stdout = output.stdout();
    assert_eq!(stdout, "/usr/bin/foo");
}
