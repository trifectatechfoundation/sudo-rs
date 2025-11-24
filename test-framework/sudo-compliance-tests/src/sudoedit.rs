use sudo_test::{
    helpers::assert_ls_output, Command, Env, EnvNoImplicit, TextFile, ETC_SUDOERS, ROOT_GROUP,
};

use crate::{
    Result, DEFAULT_EDITOR, GROUPNAME, PANIC_EXIT_CODE, SUDOERS_ALL_ALL_NOPASSWD, USERNAME,
};

mod limits;
mod sudoers;

const LOGS_PATH: &str = "/tmp/logs.txt";
const CHMOD_EXEC: &str = "555";
const EDITOR_DUMMY: &str = "#!/bin/sh
echo \"#\" >> \"$1\"";

#[test]
fn default_editor_is_usr_bin_editor() {
    let expected = "default editor was called";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                "#!/bin/sh

echo '{expected}' > {LOGS_PATH}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("sudoedit")
        .as_user(USERNAME)
        .arg("/bin/foo.sh")
        .output(&env)
        .assert_success();

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
fn creates_file_with_default_ownership_and_perms_if_it_doesnt_exist() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    Command::new("rm")
        .args(["-f", LOGS_PATH])
        .output(&env)
        .assert_success();

    Command::new("sudoedit")
        .arg("/foo.txt")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    let ls_output = Command::new("ls")
        .args(["-l", "/foo.txt"])
        .output(&env)
        .stdout();

    assert_ls_output(&ls_output, "-rw-r--r--", "root", ROOT_GROUP);
}

#[test]
fn passes_temporary_file_to_editor() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo "$@" > {LOGS_PATH}"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("sudoedit")
        .arg("/foo.txt")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    let args = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    if sudo_test::is_original_sudo() {
        assert_starts_with!(args, "/var/tmp");
    } else {
        assert_starts_with!(args, "/tmp");
    }
}

#[test]
fn temporary_file_owner_and_perms() {
    let editor_script = format!(
        r#"#!/bin/sh
ls -l "$1" > {LOGS_PATH}"#
    );

    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .group(GROUPNAME)
        .user(USERNAME)
        .file(DEFAULT_EDITOR, TextFile(editor_script).chmod(CHMOD_EXEC))
        .build();

    Command::new("sudoedit")
        .arg(ETC_SUDOERS)
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    let ls_output = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    assert_ls_output(&ls_output, "-rw-------", USERNAME, "users");
}

#[test]
fn stderr_message_when_file_is_not_modified() {
    let expected = "
Defaults !fqdn
ALL ALL=(ALL:ALL) NOPASSWD:ALL";
    let env = EnvNoImplicit(expected)
        .user(USERNAME)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
                 true",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let output = Command::new("sudoedit")
        .as_user(USERNAME)
        .arg(ETC_SUDOERS)
        .output(&env);

    output.assert_success();
    assert_contains!(output.stderr(), format!("{ETC_SUDOERS} unchanged"));

    let actual = Command::new("cat").arg(ETC_SUDOERS).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
fn editor_exits_with_a_nonzero_code() {
    let expected = "
Defaults !fqdn
ALL ALL=(ALL:ALL) NOPASSWD:ALL";
    let env = EnvNoImplicit(expected)
        .user(USERNAME)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
exit 11",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let output = Command::new("sudoedit")
        .arg(ETC_SUDOERS)
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(11);

    let actual = Command::new("cat").arg(ETC_SUDOERS).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
fn temporary_file_is_deleted_during_editing() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
rm $1",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let output = Command::new("sudoedit")
        .arg(ETC_SUDOERS)
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);
    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_contains!(stderr, format!("sudoedit: {ETC_SUDOERS} left unmodified"));
    } else {
        assert_contains!(stderr, format!("sudo: failed to read from temporary file"));
    }
}

#[test]
fn temp_file_initial_contents() {
    let expected = "
Defaults !fqdn
ALL ALL=(ALL:ALL) NOPASSWD:ALL";
    let env = EnvNoImplicit(expected)
        .user(USERNAME)
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                "#!/bin/sh
cp $1 {LOGS_PATH}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("sudoedit")
        .arg(ETC_SUDOERS)
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    assert_eq!(expected, actual);
}

#[test]
fn temporary_file_is_deleted_when_done() {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected)
        .user(USERNAME)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    Command::new("sudoedit")
        .arg("/foo.txt")
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    let output = Command::new("find")
        .args(["/tmp", "/var/tmp", "-type", "f"])
        .output(&env)
        .stdout();

    assert!(output.is_empty());
}

#[test]
#[ignore = "gh1222"]
fn temporary_file_is_deleted_when_terminated_by_signal() {
    for victim in ["editor", "child", "parent"] {
        let kill_sudo = "/root/kill-sudo.sh";
        let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
            .user(USERNAME)
            .file(
                DEFAULT_EDITOR,
                TextFile(
                    "#!/bin/sh
touch /tmp/barrier
sleep 2",
                )
                .chmod(CHMOD_EXEC),
            )
            .file(kill_sudo, include_str!("sudoedit/kill-sudoedit.sh"))
            .build();

        let child = Command::new("sudoedit")
            .arg(ETC_SUDOERS)
            .as_user(USERNAME)
            .spawn(&env);

        Command::new("sh")
            .args([kill_sudo, victim, "-TERM"])
            .output(&env)
            .assert_success();

        // the signal doesn't get propagated
        assert!(!child.wait().status().success());

        let output = Command::new("find")
            .args(["/tmp", "/var/tmp", "-type", "f", "-not", "-name", "barrier"])
            .output(&env)
            .stdout();

        assert!(output.is_empty());
    }
}

#[test]
fn does_not_panic_on_io_errors() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh

echo ' ' >> $1",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let output = Command::new("bash")
        .args([
            "-c",
            "sudoedit /etc/sudoers | true; echo \"${PIPESTATUS[0]}\"",
        ])
        .as_user(USERNAME)
        .output(&env);

    let stderr = output.stderr();
    assert!(stderr.is_empty());

    let exit_code = output.stdout().parse()?;
    assert_ne!(PANIC_EXIT_CODE, exit_code);
    assert_eq!(0, exit_code);

    Ok(())
}

#[test]
fn known_under_many_names() {
    for editor in ["sudoedit", "sudo -e", "sudo sudoedit"] {
        let command = editor.split_whitespace().next().unwrap();
        let mut args = editor.split_whitespace().skip(1).collect::<Vec<&str>>();
        let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
            .user(USERNAME)
            .file(
                DEFAULT_EDITOR,
                TextFile(format!(
                    "#!/bin/sh

echo '{editor}' > \"$1\""
                ))
                .chmod(CHMOD_EXEC),
            )
            .build();

        args.push("/bin/foo.sh");

        let output = Command::new(command)
            .args(args)
            .as_user(USERNAME)
            .output(&env);

        output.assert_success();
        if editor == "sudo sudoedit" {
            assert_contains!(output.stderr(), "sudoedit doesn't need to be run via sudo");
        }

        let actual = Command::new("cat").arg("/bin/foo.sh").output(&env).stdout();

        assert_eq!(editor, actual);
    }
}

#[test]
fn multiple_files() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh

for f in \"$@\"
do echo \"$f\" > \"$f\"
done",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let files = ["/bin/foo", "/bin/bar", "/bin/baz"];

    Command::new("sudoedit")
        .args(files)
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    for file in files {
        let actual = Command::new("cat").arg(file).output(&env).stdout();

        assert_starts_with!(
            actual[actual.rfind('/').unwrap()..],
            file[file.rfind('/').unwrap()..]
        );
    }
}

#[test]
fn can_edit_in_current_dir() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh

for f in \"$@\"
do echo \"$f\" > \"$f\"
done",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("sh")
        .args(["-c", "cd / && sudoedit foo"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    let actual = Command::new("cat").arg("/foo").output(&env).stdout();

    assert_starts_with!(actual[actual.rfind('/').unwrap()..], "/foo");
}

#[test]
fn run_editor_as_correct_user() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
id -un
id -Gn",
            )
            .chmod(CHMOD_EXEC),
        )
        .build();

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("{DEFAULT_EDITOR} && cd / && sudoedit foo"))
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();

    assert_eq!(
        output.stdout(),
        format!("{USERNAME}\nusers\n{USERNAME}\nusers")
    );
}
