use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

fn test_prompt(env: &Env, prompt_str: &str, prompt_res: &str) -> Result<()> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S -p '{prompt_str}' true"))
        .as_user(USERNAME)
        .output(env)?;

    assert!(output.status().success(), "{:?}", output);

    if sudo_test::is_original_sudo() {
        assert_eq!(output.stderr(), prompt_res);
    } else {
        assert_eq!(output.stderr(), format!("[sudo: {prompt_res}] Password: "));
    }

    Ok(())
}

#[test]
fn reads_prompt_flag() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    test_prompt(&env, "✨my fancy prompt✨", "✨my fancy prompt✨")
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
fn show_host_and_users() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .hostname("this_host")
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    test_prompt(
        &env,
        "on %H/%h: %u %U",
        "on this_host/this_host: ferris root",
    )
}

#[test]
fn show_auth_user() -> Result<()> {
    const ROOT_PASSWORD: &str = "r00t";

    let env = Env(format!("Defaults rootpw\n{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user_password("root", ROOT_PASSWORD)
        .user(User(USERNAME).password(PASSWORD))
        .user(User("user2"))
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {ROOT_PASSWORD} | sudo -S -p '%u %U %p' -u user2 true"
        ))
        .as_user(USERNAME)
        .output(&env)?;

    assert!(output.status().success(), "{:?}", output);

    if sudo_test::is_original_sudo() {
        assert_eq!(output.stderr(), "ferris user2 root");
    } else {
        assert_eq!(output.stderr(), "[sudo: ferris user2 root] Password: ");
    }

    Ok(())
}

#[test]
fn invalid_flag() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    test_prompt(&env, "%A", "%A")
}

#[test]
fn ends_with_percent() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    test_prompt(&env, "foo %", "foo %")
}

#[test]
fn percent_escape() -> Result<()> {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build()?;

    test_prompt(&env, "%%u", "%u")
}
