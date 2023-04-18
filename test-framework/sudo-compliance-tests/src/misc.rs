use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "snapshots/misc",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn user_not_in_passwd_database_cannot_use_sudo() -> Result<()> {
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD).build()?;

    let output = Command::new("sudo")
        .arg("true")
        .as_user_id(1000)
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        assert_snapshot!(output.stderr());
    }

    Ok(())
}
