use sudo_test::{Command, Env, TextFile, User};

use crate::{Result, PASSWORD, SUDOERS_USER_ALL_ALL, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/flag_login",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[track_caller]
fn test_prompt(prompt: &str) -> Result<()> {
    todo!()
}

#[test]
fn reads_prompt_flag() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S -p '✨my fancy prompt✨' true"
        ))
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success(), "{:?}", output);

    if sudo_test::is_original_sudo() {
        assert_eq!(output.stderr(), "✨my fancy prompt✨");
    } else {
        assert_eq!(output.stderr(), "[sudo: ✨my fancy prompt✨] Password: ");
    }

    Ok(())
}

#[test]
fn empty_prompt_disables_prompt() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S -p '' true"))
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success(), "{:?}", output);

    assert_eq!(output.stderr(), "");

    Ok(())
}

#[test]
fn show_user() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .hostname("this_host")
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S -p 'on %H/%h: %u %U' true"
        ))
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success(), "{:?}", output);

    if sudo_test::is_original_sudo() {
        assert_eq!(output.stderr(), "on this_host/this_host: ferris root");
    } else {
        assert_eq!(output.stderr(), "[sudo: on this_host/this_host: ferris root] Password: ");
    }

    Ok(())
}

// FIXME test the various % flags
// FIXME test invalid % flags
// FIXME test prompt ending with %
// FIXME test prompt with %%
