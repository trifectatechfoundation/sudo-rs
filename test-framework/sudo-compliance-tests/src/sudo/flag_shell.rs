use std::collections::HashMap;

use sudo_test::{Command, Env, TextFile};

use crate::{SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/flag_shell",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn if_shell_env_var_is_not_set_then_uses_the_invoking_users_shell_in_passwd_database() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).user(USERNAME).build();

    // Make sure the shell of root doesn't match the shell of ferris
    let output = Command::new("chsh")
        .args(["-s", "/non/existent"])
        .output(&env);
    output.assert_success();

    let getent_passwd = Command::new("getent").arg("passwd").output(&env).stdout();
    let user_to_shell = parse_getent_passwd_output(&getent_passwd);
    let target_users_shell = user_to_shell["root"];
    let invoking_users_shell = user_to_shell["ferris"];

    assert_ne!(target_users_shell, invoking_users_shell);

    let output = Command::new("env")
        .args(["-u", "SHELL", "sudo", "-s", "echo", "$0"])
        .as_user(USERNAME)
        .output(&env)
        .stdout();

    // /bin will be resolved to /usr/bin by sudo-rs
    let output = output.replace("/usr/bin", "/bin");

    assert_eq!(invoking_users_shell, output);
}

#[test]
fn if_shell_env_var_is_set_then_uses_it() {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $0";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build();

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s"])
        .output(&env)
        .stdout();

    assert_eq!(shell_path, output);
}

#[test]
fn shell_is_partially_canonicalized() {
    let shell_path = "/tmp/mysh";
    let bin_link = "/tmp/bin";
    let env = Env("ALL ALL=(ALL:ALL) NOPASSWD: /bin/sh").build();

    Command::new("ln")
        .args(["-s", "/bin/sh", shell_path])
        .output(&env)
        .assert_success();

    Command::new("ln")
        .args(["-s", "/bin", bin_link])
        .output(&env)
        .assert_success();

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s", "true"])
        .output(&env);

    assert!(!output.status().success());

    let output = Command::new("env")
        .arg(format!("SHELL={bin_link}/sh"))
        .args(["sudo", "-s", "true"])
        .output(&env);

    output.assert_success();

    let output = Command::new("env")
        .arg("SHELL=/bin/ls")
        .args(["sudo", "-s", "true"])
        .output(&env);

    assert!(!output.status().success());
}

#[test]
fn argument_is_invoked_with_dash_c_flag() {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build();

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s", "argument"])
        .output(&env)
        .stdout();

    assert_eq!("-c argument", output);
}

#[test]
fn arguments_are_concatenated_with_whitespace() {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build();

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s", "a", "b"])
        .output(&env)
        .stdout();

    assert_eq!("-c a b", output);
}

#[test]
fn arguments_are_properly_distinguished() {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
for arg in \"$@\"; do echo -n \"{$arg}\"; done";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build();

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s", "a b", "c d"])
        .output(&env)
        .stdout();

    assert_eq!("{-c}{a\\ b c\\ d}", output);
}

#[test]
fn arguments_are_escaped_with_backslashes() {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build();

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s", "'", "\"", "a b"])
        .output(&env)
        .stdout();

    assert_eq!(r#"-c \' \" a\ b"#, output);
}

#[test]
fn alphanumerics_underscores_hyphens_and_dollar_signs_are_not_escaped() {
    let shell_path = "/root/my-shell";
    let my_shell = "#!/bin/sh
echo $@";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile(my_shell).chmod("100"))
        .build();

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s", "a", "1", "_", "-", "$", "$VAR", "${VAR}"])
        .output(&env)
        .stdout();

    assert_eq!(r"-c a 1 _ - $ $VAR $\{VAR\}", output);
}

#[test]
fn shell_is_not_invoked_as_a_login_shell() {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build();

    let actual = Command::new("env")
        .args(["SHELL=/bin/sh", "sudo", "-s", "echo", "$0"])
        .output(&env)
        .stdout();

    // man bash says "A login shell is one whose first character of argument zero is a -"
    assert_ne!("-sh", actual);

    // sudo-rs and ogsudo will show paths differently; and the location of sh is different on
    // modern Debian (/usr/bin/sh, symlinked as /bin/sh) and modern FreeBSD (/bin/sh); all we need
    // to check is that it is executed without a dash in front.
    assert_contains!(actual, "/bin/sh");
}

#[test]
fn shell_does_not_exist() {
    let shell_path = "/root/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build();

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s"])
        .output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "'/root/my-shell': command not found");
    }
}

#[test]
fn shell_is_not_executable() {
    let shell_path = "/root/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile("#!/bin/sh").chmod("000"))
        .build();

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s"])
        .output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "Permission denied");
    }
}

#[test]
fn shell_with_open_permissions_is_accepted() {
    let shell_path = "/tmp/my-shell";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(shell_path, TextFile("#!/bin/sh").chmod("777"))
        .build();

    let output = Command::new("env")
        .arg(format!("SHELL={shell_path}"))
        .args(["sudo", "-s"])
        .output(&env);

    output.assert_success();
}

type UserToShell<'a> = HashMap<&'a str, &'a str>;

fn parse_getent_passwd_output(passwd: &str) -> UserToShell<'_> {
    const ERROR: &str = "malformed `getent passwd` output";
    let mut map = HashMap::new();
    for line in passwd.lines() {
        let Some((user, _)) = line.split_once(':') else {
            panic!("{ERROR}");
        };
        let Some((_, shell)) = line.rsplit_once(':') else {
            panic!("{ERROR}");
        };

        map.insert(user, shell);
    }
    map
}
