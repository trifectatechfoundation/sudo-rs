use sudo_test::{Command, Env, TextFile, User};

use crate::{PASSWORD, USERNAME};

use super::MAX_PASSWORD_SIZE;

const CHMOD_EXEC: &str = "555";

fn generate_askpass(password: &str) -> TextFile {
    TextFile(format!("#!/bin/sh\necho {password}")).chmod(CHMOD_EXEC)
}

#[test]
fn correct_password() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .file("/bin/askpass", generate_askpass(PASSWORD))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sh")
        .args(["-c", "SUDO_ASKPASS=/bin/askpass sudo -A true"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn incorrect_password() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .file("/bin/askpass", generate_askpass("incorrect-password"))
        .user(User(USERNAME).password("strong-password"))
        .build();

    let output = Command::new("sh")
        .args(["-c", "SUDO_ASKPASS=/bin/askpass sudo -A true"])
        .as_user(USERNAME)
        .output(&env);
    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "incorrect password attempt"
    } else {
        "Authentication failed, try again."
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn no_password() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .file("/bin/askpass", TextFile("#!/bin/sh").chmod(CHMOD_EXEC))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let output = Command::new("sh")
        .args(["-c", "SUDO_ASKPASS=/bin/askpass sudo -A true"])
        .as_user(USERNAME)
        .output(&env);
    output.assert_exit_code(1);

    let diagnostic = if sudo_test::is_original_sudo() {
        "no password was provided"
    } else {
        "Authentication required but not attempted"
    };
    assert_contains!(output.stderr(), diagnostic);
}

#[test]
fn longest_possible_password_works() {
    let password = "a".repeat(MAX_PASSWORD_SIZE);

    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file("/bin/askpass", generate_askpass(&password))
        .user(User(USERNAME).password(&password))
        .build();

    Command::new("sh")
        .args(["-c", "SUDO_ASKPASS=/bin/askpass sudo -A true"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn input_longer_than_max_pam_response_size_is_handled_gracefully() {
    let input = "a".repeat(5 * MAX_PASSWORD_SIZE / 2);

    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file("/bin/askpass", generate_askpass(&input))
        .user(USERNAME)
        .build();

    let output = Command::new("sh")
        .args(["-c", "SUDO_ASKPASS=/bin/askpass sudo -A true"])
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_contains!(stderr, "sudo: 3 incorrect password attempts");
    } else {
        assert_contains!(stderr, "Incorrect authentication attempt");
        assert_not_contains!(stderr, "panic");
    }
}

#[test]
fn input_longer_than_password_should_not_be_accepted_as_correct_password() {
    let password = "a".repeat(MAX_PASSWORD_SIZE);

    let input_sizes = [MAX_PASSWORD_SIZE + 1, MAX_PASSWORD_SIZE + 2];

    for input_size in input_sizes {
        let input = "a".repeat(input_size);

        let env = Env("ALL ALL=(ALL:ALL) ALL")
            .file("/bin/askpass", generate_askpass(&input))
            .user(User(USERNAME).password(password.clone()))
            .build();

        let output = Command::new("sh")
            .args(["-c", "SUDO_ASKPASS=/bin/askpass sudo -A true"])
            .as_user(USERNAME)
            .output(&env);

        output.assert_exit_code(1);

        let stderr = output.stderr();
        if sudo_test::is_original_sudo() {
            assert_contains!(stderr, "sudo: 3 incorrect password attempt");
        } else {
            assert_contains!(stderr, "Incorrect authentication attempt");
        }
    }
}

#[test]
fn sudo_askpass_not_set() {
    let env = Env("ALL ALL=(ALL:ALL) ALL").user(User(USERNAME)).build();

    let output = Command::new("sudo")
        .args(["-A", "true"])
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_contains!(
            stderr,
            "no askpass program specified, try setting SUDO_ASKPASS"
        );
    } else {
        assert_contains!(stderr, "No askpass program specified in SUDO_ASKPASS");
    }
}

#[test]
fn sudo_askpass_not_absolute_path() {
    let env = Env("ALL ALL=(ALL:ALL) ALL").user(User(USERNAME)).build();

    let output = Command::new("sh")
        .args(["-c", "SUDO_ASKPASS=askpass sudo -A true"])
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_contains!(stderr, "unable to run askpass: No such file or directory");
    } else {
        assert_contains!(stderr, "Askpass program 'askpass' is not an absolute path");
    }
}

#[test]
fn askpass_not_executable() {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file("/bin/askpass", format!("#!/bin/sh\necho {PASSWORD}"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let output = Command::new("sh")
        .args(["-c", "SUDO_ASKPASS=/bin/askpass sudo -A true"])
        .as_user(USERNAME)
        .output(&env);

    output.assert_exit_code(1);

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_contains!(stderr, "unable to run /bin/askpass: Permission denied");
    } else {
        assert_contains!(
            stderr,
            "Failed to run askpass program /bin/askpass: Permission denied (os error 13)"
        );
    }
}

#[test]
fn askpass_exit_code_ignored() {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file(
            "/bin/askpass",
            TextFile(format!("#!/bin/sh\necho {PASSWORD}\nfalse")).chmod(CHMOD_EXEC),
        )
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sh")
        .args(["-c", "SUDO_ASKPASS=/bin/askpass sudo -A true"])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();
}

#[test]
fn prompt_given_as_argument() {
    let env = Env("ALL ALL=(ALL:ALL) ALL")
        .file(
            "/bin/askpass",
            TextFile(format!(
                "#!/bin/sh\necho \"$1\" > /tmp/prompt\necho {PASSWORD}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .user(User(USERNAME).password(PASSWORD))
        .build();

    Command::new("sh")
        .args([
            "-c",
            "SUDO_ASKPASS=/bin/askpass sudo -A -p 'my fancy prompt' true",
        ])
        .as_user(USERNAME)
        .output(&env)
        .assert_success();

    let output = Command::new("cat").arg("/tmp/prompt").output(&env);
    assert_contains!(output.stdout(), "my fancy prompt");
}
