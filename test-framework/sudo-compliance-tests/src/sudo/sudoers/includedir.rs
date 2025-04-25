use sudo_test::{Command, Directory, Env, TextFile, ETC_DIR, ETC_PARENT_DIR};

use crate::{SUDOERS_ALL_ALL_NOPASSWD, SUDOERS_USER_ALL_ALL, SUDOERS_USER_ALL_NOPASSWD, USERNAME};

#[test]
fn absolute_path() {
    let env = Env(format!("@includedir {ETC_DIR}/sudoers.d"))
        .file(format!("{ETC_DIR}/sudoers.d/a"), SUDOERS_ALL_ALL_NOPASSWD)
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn relative_path() {
    let env = Env("@includedir sudoers.d")
        .file(format!("{ETC_DIR}/sudoers.d/a"), SUDOERS_ALL_ALL_NOPASSWD)
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn ignores_files_with_names_ending_in_tilde() {
    let env = Env(format!("@includedir {ETC_DIR}/sudoers.d"))
        .file(format!("{ETC_DIR}/sudoers.d/a~"), SUDOERS_ALL_ALL_NOPASSWD)
        .build();

    let output = Command::new("sudo").arg("true").output(&env);

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostic = if sudo_test::is_original_sudo() {
        "root is not in the sudoers file"
    } else {
        "I'm sorry root. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn ignores_files_with_names_that_contain_a_dot() {
    let env = Env(format!("@includedir {ETC_DIR}/sudoers.d"))
        .file(format!("{ETC_DIR}/sudoers.d/a."), SUDOERS_ALL_ALL_NOPASSWD)
        .file(format!("{ETC_DIR}/sudoers.d/.b"), SUDOERS_ALL_ALL_NOPASSWD)
        .file(format!("{ETC_DIR}/sudoers.d/c.d"), SUDOERS_ALL_ALL_NOPASSWD)
        .build();

    let output = Command::new("sudo").arg("true").output(&env);

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostic = if sudo_test::is_original_sudo() {
        "root is not in the sudoers file"
    } else {
        "I'm sorry root. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn directory_does_not_exist_is_not_fatal() {
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "@includedir /etc/does-not-exist"]).build();

    let output = Command::new("sudo").arg("true").output(&env);

    output.assert_success();
    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert!(stderr.is_empty());
    } else {
        assert_contains!(stderr, "cannot open sudoers file /etc/does-not-exist");
    }
}

#[test]
fn loads_files_in_lexical_order() {
    let env = Env(format!("@includedir {ETC_DIR}/sudoers.d"))
        .file(format!("{ETC_DIR}/sudoers.d/a"), "ALL ALL=(ALL:ALL) ALL")
        .file(
            format!("{ETC_DIR}/sudoers.d/b"),
            "ALL ALL=(ALL:ALL) NOPASSWD: ALL",
        )
        .user(USERNAME)
        .build();

    Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn ignores_and_warns_about_file_with_bad_perms() {
    let env = Env([
        SUDOERS_USER_ALL_NOPASSWD,
        &format!("@includedir {ETC_DIR}/sudoers.d"),
    ])
    .file(
        format!("{ETC_DIR}/sudoers.d/a"),
        // if this was NOT ignored, then the `sudo true` below would fail because no password
        // was provided
        TextFile(SUDOERS_USER_ALL_ALL).chmod("777"),
    )
    .user(USERNAME)
    .build();

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();
    let diagnostic = if sudo_test::is_original_sudo() {
        format!("{ETC_DIR}/sudoers.d/a is world writable")
    } else {
        format!("{ETC_DIR}/sudoers.d/a cannot be world-writable")
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn ignores_and_warns_about_file_with_bad_ownership() {
    let env = Env([
        SUDOERS_USER_ALL_NOPASSWD,
        &format!("@includedir {ETC_DIR}/sudoers.d"),
    ])
    .file(
        format!("{ETC_DIR}/sudoers.d/a"),
        // if this was NOT ignored, then the `sudo true` below would fail because no password
        // was provided
        TextFile(SUDOERS_USER_ALL_ALL).chown(USERNAME),
    )
    .user(USERNAME)
    .build();

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();
    let diagnostic = if sudo_test::is_original_sudo() {
        if cfg!(target_os = "freebsd") {
            format!("{ETC_DIR}/sudoers.d/a is owned by uid 1001, should be 0")
        } else {
            format!("{ETC_DIR}/sudoers.d/a is owned by uid 1000, should be 0")
        }
    } else {
        format!("{ETC_DIR}/sudoers.d/a must be owned by root")
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn include_loop() {
    let env = Env([
        SUDOERS_USER_ALL_NOPASSWD,
        &format!("@includedir {ETC_DIR}/sudoers.d"),
    ])
    .file(
        format!("{ETC_DIR}/sudoers.d/a"),
        TextFile(format!("@include {ETC_DIR}/sudoers.d/a")),
    )
    .user(USERNAME)
    .build();

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();
    let diagnostic = if sudo_test::is_original_sudo() {
        format!("{ETC_DIR}/sudoers.d/a: too many levels of includes")
    } else {
        format!("include file limit reached opening '{ETC_DIR}/sudoers.d/a'")
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn statements_prior_to_include_loop_are_evaluated() {
    let env = Env([
        SUDOERS_USER_ALL_ALL,
        &format!("@includedir {ETC_DIR}/sudoers.d"),
    ])
    .file(
        format!("{ETC_DIR}/sudoers.d/a"),
        TextFile(format!(
            // if this first line was ignored the `sudo true` below would fail because a
            // password was not provided
            "{SUDOERS_USER_ALL_NOPASSWD}
@include {ETC_DIR}/sudoers.d/a"
        )),
    )
    .user(USERNAME)
    .build();

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();

    let diagnostic = if sudo_test::is_original_sudo() {
        format!("{ETC_DIR}/sudoers.d/a: too many levels of includes")
    } else {
        format!("include file limit reached opening '{ETC_DIR}/sudoers.d/a'")
    };

    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn whitespace_in_name_escaped() {
    let env = Env(r"@includedir /etc/sudo\ ers.d")
        .directory(r#"/etc/sudo ers.d"#)
        .file(r#"/etc/sudo ers.d/a"#, SUDOERS_ALL_ALL_NOPASSWD)
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn whitespace_in_name_double_quotes() {
    let env = Env(r#"@includedir "/etc/sudo ers.d" "#)
        .directory(r#"/etc/sudo ers.d"#)
        .file(r#"/etc/sudo ers.d/a"#, SUDOERS_ALL_ALL_NOPASSWD)
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "zfs on freebsd doesn't allow creating files with backslashes"
)]
fn backslash_in_name_escaped() {
    let env = Env(r"@includedir /etc/sudo\\ers.d")
        .directory(r"/etc/sudo\ers.d")
        .file(r"/etc/sudo\ers.d/a", SUDOERS_ALL_ALL_NOPASSWD)
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
#[cfg_attr(
    target_os = "freebsd",
    ignore = "zfs on freebsd doesn't allow creating files with backslashes"
)]
fn backslash_in_name_double_quotes() {
    let env = Env(r#"@includedir "/etc/sudo\ers.d""#)
        .directory(r"/etc/sudo\ers.d")
        .file(r"/etc/sudo\ers.d/a", SUDOERS_ALL_ALL_NOPASSWD)
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn old_pound_syntax() {
    let env = Env(format!("#includedir {ETC_DIR}/sudoers.d"))
        .file(format!("{ETC_DIR}/sudoers.d/a"), SUDOERS_ALL_ALL_NOPASSWD)
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn no_hostname_expansion() {
    let hostname = "ship";
    let env = Env(format!("@includedir {ETC_DIR}/sudoers.%h"))
        .directory(format!("{ETC_DIR}/sudoers.{hostname}"))
        .file(
            format!("{ETC_DIR}/sudoers.{hostname}/a"),
            SUDOERS_ALL_ALL_NOPASSWD,
        )
        .build();

    let output = Command::new("sudo").arg("true").output(&env);

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostic = if sudo_test::is_original_sudo() {
        "root is not in the sudoers file"
    } else {
        "I'm sorry root. I'm afraid I can't do that"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn ignores_directory_with_bad_perms() {
    let env = Env(format!("@includedir {ETC_DIR}/sudoers2.d"))
        .directory(Directory(format!("{ETC_DIR}/sudoers2.d")).chmod("777"))
        .file(format!("{ETC_DIR}/sudoers2.d/a"), SUDOERS_ALL_ALL_NOPASSWD)
        .build();

    let output = Command::new("sudo").arg("true").output(&env);

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostics = if sudo_test::is_original_sudo() {
        [
            format!("sudo: {ETC_DIR}/sudoers2.d is world writable"),
            "root is not in the sudoers file".to_owned(),
        ]
    } else {
        [
            format!("{ETC_DIR}/sudoers2.d cannot be world-writable"),
            "I'm sorry root. I'm afraid I can't do that".to_owned(),
        ]
    };
    for diagnostic in diagnostics {
        assert_contains!(output.stderr(), diagnostic);
    }
}

#[test]
fn ignores_directory_with_bad_ownership() {
    let env = Env(format!("@includedir {ETC_DIR}/sudoers2.d"))
        .directory(Directory(format!("{ETC_DIR}/sudoers2.d")).chown(USERNAME))
        .file(format!("{ETC_DIR}/sudoers2.d/a"), SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .build();

    let output = Command::new("sudo").arg("true").output(&env);

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let diagnostics = if sudo_test::is_original_sudo() {
        [
            if cfg!(target_os = "freebsd") {
                format!("sudo: {ETC_DIR}/sudoers2.d is owned by uid 1001, should be 0")
            } else {
                format!("sudo: {ETC_DIR}/sudoers2.d is owned by uid 1000, should be 0")
            },
            "root is not in the sudoers file".to_owned(),
        ]
    } else {
        [
            format!("{ETC_DIR}/sudoers2.d must be owned by root"),
            "I'm sorry root. I'm afraid I can't do that".to_owned(),
        ]
    };

    for diagnostic in diagnostics {
        assert_contains!(output.stderr(), diagnostic);
    }
}

#[test]
fn relative_path_parent_directory() {
    let env = Env("@includedir ../sudoers.d")
        .directory(format!("{ETC_PARENT_DIR}/sudoers.d"))
        .file(
            format!("{ETC_PARENT_DIR}/sudoers.d/a"),
            SUDOERS_ALL_ALL_NOPASSWD,
        )
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn relative_path_grandparent_directory() {
    // base path is `/etc/` so grandparent does not exist
    let env = Env(if cfg!(target_os = "freebsd") {
        "@includedir ../../../../sudoers.d"
    } else {
        "@includedir ../../sudoers.d"
    })
    .directory("/sudoers.d")
    .file("/sudoers.d/a", SUDOERS_ALL_ALL_NOPASSWD)
    .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn relative_path_dot_slash() {
    // base path is `/etc/` so grandparent does not exist
    let env = Env("@includedir ./sudoers.d")
        .file(format!("{ETC_DIR}/sudoers.d/a"), SUDOERS_ALL_ALL_NOPASSWD)
        .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}
