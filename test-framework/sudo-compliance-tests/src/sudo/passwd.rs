use sudo_test::{BIN_LS, BIN_TRUE, Command, Env};

use crate::{SUDOERS_NO_LECTURE, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => if cfg!(target_os = "linux") {
                vec![(r"[[:xdigit:]]{12}", "[host]")]
            } else {
                vec![
                    (r"[[:xdigit:]]{12}", "[host]"),
                    ("Password:", "[sudo] password for ferris: "),
                ]
            },
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/passwd",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn explicit_passwd_overrides_nopasswd() {
    let env = Env([
        format!("ALL ALL=(ALL:ALL) NOPASSWD: {BIN_TRUE}, PASSWD: {BIN_LS}"),
        SUDOERS_NO_LECTURE.to_owned(),
    ])
    .user(USERNAME)
    .build();

    let output = Command::new("sudo")
        .arg("true")
        .as_user(USERNAME)
        .output(&env);

    output.assert_success();

    let second_output = Command::new("sudo")
        .args(["-S", "ls"])
        .as_user(USERNAME)
        .output(&env);

    second_output.assert_exit_code(1);

    let stderr = second_output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "Authentication required but not attempted");
    }
}
