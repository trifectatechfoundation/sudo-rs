use sudo_test::{Command, Env, User};

use crate::{PASSWORD, USERNAME};

fn test_prompt(env: &Env, prompt_str: &str, prompt_res: &str, mode_flag: &str) {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | SUDO_PROMPT='{prompt_str}' sudo -S {mode_flag}"
        ))
        .as_user(USERNAME)
        .output(env);

    output.assert_success();

    if sudo_test::is_original_sudo() {
        assert_eq!(output.stderr(), prompt_res);
    } else {
        assert_eq!(output.stderr(), format!("[sudo: {prompt_res}] Password: "));
    }
}

#[test]
fn reads_prompt_var() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .build();

    test_prompt(&env, "✨my fancy prompt✨", "✨my fancy prompt✨", "-v")
}

#[test]
fn show_host_and_users_for_list_and_verify() {
    let env = Env(format!("{USERNAME}    ALL=(ALL:ALL) ALL"))
        .hostname("this_host.domain")
        .user(User(USERNAME).password(PASSWORD))
        .build();

    for mode in ["-v", "-l"] {
        test_prompt(
            &env,
            "on %H/%h: %u %U",
            "on this_host.domain/this_host: ferris root",
            mode,
        )
    }
}
