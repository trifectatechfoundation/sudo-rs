use sudo_test::{Command, Env, User};

use crate::{Result, PASSWORD, USERNAME};

#[test]
fn it_works() -> Result<()> {
    let hostname = "container";
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .hostname(hostname)
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("echo {PASSWORD} | sudo -S -l; sudo -l"))
        .as_user(USERNAME)
        .output(&env)?;

    let stdout = output.stdout()?;
    let it_worked = stdout
        .lines()
        .filter(|line| {
            line.starts_with(&format!(
                "User {USERNAME} may run the following commands on {hostname}:"
            ))
        })
        .count();

    assert_eq!(2, it_worked);

    Ok(())
}

#[test]
fn credential_shared_with_non_list_sudo() -> Result<()> {
    let hostname = "container";
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .hostname(hostname)
        .build()?;

    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S -l 2>/dev/null >/tmp/stdout1.txt; sudo true"
        ))
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    let stdout1 = Command::new("cat")
        .arg("/tmp/stdout1.txt")
        .output(&env)?
        .stdout()?;

    assert_contains!(
        stdout1,
        format!("User {USERNAME} may run the following commands on {hostname}:")
    );

    Ok(())
}

#[test]
fn flag_reset_timestamp() -> Result<()> {
    let hostname = "container";
    let env = Env(format!("{USERNAME} ALL=(ALL:ALL) ALL"))
        .user(User(USERNAME).password(PASSWORD))
        .hostname(hostname)
        .build()?;

    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "echo {PASSWORD} | sudo -S -l 2>/dev/null >/tmp/stdout1.txt; sudo -k; sudo -l"
        ))
        .as_user(USERNAME)
        .output(&env)?;

    let stdout1 = Command::new("cat")
        .arg("/tmp/stdout1.txt")
        .output(&env)?
        .stdout()?;

    assert_contains!(
        stdout1,
        format!("User {USERNAME} may run the following commands on {hostname}:")
    );

    assert!(!output.status().success());
    let diagnostic = if sudo_test::is_original_sudo() {
        "sudo: a password is required"
    } else {
        "sudo: Authentication failed"
    };
    assert_contains!(output.stderr(), diagnostic);

    Ok(())
}
