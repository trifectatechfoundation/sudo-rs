use sudo_test::{Command, Env, TextFile};

use crate::{helpers, Result, SUDOERS_ALL_ALL_NOPASSWD, USERNAME};

#[test]
fn can_find_command_not_visible_to_regular_user() -> Result<()> {
    let path = "/root/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .user(USERNAME)
        .file(path, TextFile("#!/bin/sh").chmod("100"))
        .build()?;

    Command::new("sh")
        .args(["-c", "export PATH=/root; cd /; /usr/bin/sudo my-script"])
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn when_path_is_unset_does_not_search_in_default_path_set_for_command_execution() -> Result<()> {
    let path = "/usr/bin/my-script";
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(path, TextFile("#!/bin/sh"))
        .build()?;

    let default_path = Command::new("sh")
        .args(["-c", "unset PATH; /usr/bin/sudo /usr/bin/printenv PATH"])
        .exec(&env)?
        .stdout()?;

    // sanity check that `/usr/bin` is in sudo's default PATH
    let default_path = helpers::parse_path(&default_path);
    assert!(default_path.contains("/usr/bin"));

    let output = Command::new("sh")
        .args(["-c", "unset PATH; /usr/bin/sudo my-script"])
        .exec(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    if sudo_test::is_original_sudo() {
        insta::assert_snapshot!(output.stderr());
    }

    Ok(())
}
