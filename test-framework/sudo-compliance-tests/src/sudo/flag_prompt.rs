use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

fn test_prompt(env: &Env, prompt_str: &str, prompt_res: &str) {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S -p '{prompt_str}' true"))
        .as_user(USERNAME)
        .output(env);

    assert!(output.status().success(), "{:?}", output);

    if sudo_test::is_original_sudo() {
        assert_eq!(output.stderr(), prompt_res);
    } else {
        assert_eq!(output.stderr(), format!("[sudo: {prompt_res}] Password: "));
    }
}

#[test]
fn reads_prompt_flag() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    test_prompt(&env, "✨my fancy prompt✨", "✨my fancy prompt✨")
}

#[test]
fn empty_prompt_disables_prompt() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S -p '' true"))
        .as_user(USERNAME)
        .output(&env);

    assert!(output.status().success(), "{:?}", output);

    assert_eq!(output.stderr(), "");
}

#[test]
fn show_host_and_users() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .hostname("this_host.domain")
        .user(User(USERNAME).password(PASSWORD))
        .build();

    test_prompt(
        &env,
        "on %H/%h: %u %U",
        "on this_host.domain/this_host: ferris root",
    )
}

#[test]
fn show_auth_user() {
    const ROOT_PASSWORD: &str = "r00t";

    let env = Env(format!("Defaults rootpw\n{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user_password("root", ROOT_PASSWORD)
        .user(User(USERNAME).password(PASSWORD))
        .user(User("user2"))
        .build();

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {ROOT_PASSWORD} | sudo -S -p '%u %U %p' -u user2 true"
        ))
        .as_user(USERNAME)
        .output(&env);

    assert!(output.status().success(), "{:?}", output);

    if sudo_test::is_original_sudo() {
        assert_eq!(output.stderr(), "ferris user2 root");
    } else {
        assert_eq!(output.stderr(), "[sudo: ferris user2 root] Password: ");
    }
}

#[test]
fn invalid_flag() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    test_prompt(&env, "%A", "%A")
}

#[test]
fn ends_with_percent() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    test_prompt(&env, "foo %", "foo %")
}

#[test]
fn percent_escape() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    test_prompt(&env, "%%u", "%u")
}
