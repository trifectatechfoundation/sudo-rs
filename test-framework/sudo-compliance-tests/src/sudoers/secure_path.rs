use sudo_test::{Command, Env, TextFile};

use crate::{helpers, Result, SUDOERS_ALL_ALL_NOPASSWD};

#[test]
fn if_unset_searches_program_in_invoking_users_path() -> Result<()> {
    let path = "/root/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(path, TextFile("#!/bin/sh").chmod("100"))
        .build()?;

    Command::new("sh")
        .args(["-c", "export PATH=/root; cd /; /usr/bin/sudo my-script"])
        .exec(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn if_set_searches_program_in_secure_path() -> Result<()> {
    let path = "/root/my-script";
    let env = Env("\
Defaults secure_path=.:/root
ALL ALL=(ALL:ALL) NOPASSWD: ALL")
    .file(path, TextFile("#!/bin/sh").chmod("100"))
    .build()?;

    let relative_path = "unset PATH; cd /bin; /usr/bin/sudo true";
    let absolute_path = "unset PATH; cd /; /usr/bin/sudo my-script";

    let scripts = [relative_path, absolute_path];

    for script in scripts {
        println!("{script}");

        Command::new("sh")
            .args(["-c", script])
            .exec(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[ignore]
#[test]
fn if_set_it_becomes_the_path_set_for_program_execution() -> Result<()> {
    let secure_path = ".:/root";
    let env = Env(format!(
        "Defaults secure_path={secure_path}
ALL ALL=(ALL:ALL) NOPASSWD: ALL"
    ))
    .build()?;

    let user_path_set = "cd /; sudo /usr/bin/env";
    let user_path_unset = "unset PATH; cd /; /usr/bin/sudo /usr/bin/env";
    let scripts = [user_path_set, user_path_unset];

    for script in scripts {
        println!("{script}");

        let env_output = Command::new("sh")
            .args(["-c", script])
            .exec(&env)?
            .stdout()?;

        let env_output = helpers::parse_env_output(&env_output)?;
        assert_eq!(&secure_path, &env_output["PATH"]);
    }

    Ok(())
}
