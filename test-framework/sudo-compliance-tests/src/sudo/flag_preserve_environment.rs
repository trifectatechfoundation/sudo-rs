use sudo_test::{Command, Env};

use crate::{helpers, SUDOERS_ALL_ALL_NOPASSWD};

#[test]
fn var_is_preserved() {
    let name = "SHOULD_BE_PRESERVED";
    let value = "42";
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults setenv"]).build();

    let stdout = Command::new("env")
        .arg(format!("{name}={value}"))
        .args(["sudo", &format!("--preserve-env={name}"), "env"])
        .output(&env)
        .stdout();
    let sudo_env = helpers::parse_env_output(&stdout);

    assert_eq!(Some(value), sudo_env.get(name).copied());
}

#[test]
fn env_var_overrides_preserve() {
    let name = "SHOULD_BE_PRESERVED";
    let value = "42";
    let other_value = "37";
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults setenv"]).build();

    let stdout = Command::new("env")
        .arg(format!("{name}={value}"))
        .args([
            "sudo",
            &format!("--preserve-env={name}"),
            &format!("{name}={other_value}"),
            "env",
        ])
        .output(&env)
        .stdout();
    let sudo_env = helpers::parse_env_output(&stdout);

    assert_eq!(Some(other_value), sudo_env.get(name).copied());
}

#[test]
fn preserve_overrides_env_var() {
    let name = "SHOULD_BE_PRESERVED";
    let value = "42";
    let other_value = "37";
    let env = Env([SUDOERS_ALL_ALL_NOPASSWD, "Defaults setenv"]).build();

    let stdout = Command::new("env")
        .arg(format!("{name}={value}"))
        .args([
            "sudo",
            &format!("{name}={other_value}"),
            &format!("--preserve-env={name}"),
            "env",
        ])
        .output(&env)
        .stdout();
    let sudo_env = helpers::parse_env_output(&stdout);

    assert_eq!(Some(value), sudo_env.get(name).copied());
}
