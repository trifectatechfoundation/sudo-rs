use sudo_test::{Command, Env, User};

use crate::{Result, GROUPNAME, PASSWORD, SUDOERS_NO_LECTURE, USERNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![(r"[[:xdigit:]]{12}", "[host]")],
            prepend_module_to_snapshot => false,
            snapshot_path => "../snapshots/sudoers/runas_alias",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn runas_alias_works() -> Result<()> {
    let env = Env([
        "Runas_Alias OP = root, operator",
        "root ALL=(ALL:ALL) NOPASSWD: ALL",
        &format!("{USERNAME} ALL = (OP) ALL"),
    ])
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-u", "root", "-S", "true"])
            .as_user(user)
            .stdin(PASSWORD)
            .output(&env)?
            .assert_success()?;
    }
    Command::new("sudo")
        .args(["-S", "true"])
        .as_user("root")
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn underscore() -> Result<()> {
    let env = Env([
        "Runas_Alias UNDER_SCORE = root, operator",
        "root ALL=(ALL:ALL) NOPASSWD: ALL",
        &format!("{USERNAME} ALL = (UNDER_SCORE) ALL"),
    ])
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-u", "root", "-S", "true"])
            .as_user(user)
            .stdin(PASSWORD)
            .output(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn runas_alias_negation() -> Result<()> {
    let env = Env([
        "Runas_Alias OP = root, operator",
        "root ALL = (ALL:ALL) NOPASSWD: ALL",
        &format!("{USERNAME} ALL = (!OP) ALL"),
        SUDOERS_NO_LECTURE,
    ])
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    let output = Command::new("sudo")
        .args(["-u", "root", "-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            format!("authentication failed: I'm sorry {USERNAME}. I'm afraid I can't do that")
        );
    }

    Ok(())
}

#[test]
fn negation_on_user() -> Result<()> {
    let env = Env([
        "Runas_Alias OP = !root, operator",
        "root ALL = (ALL:ALL) NOPASSWD: ALL",
        &format!("{USERNAME} ALL = (OP) ALL"),
        SUDOERS_NO_LECTURE,
    ])
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    let output = Command::new("sudo")
        .args(["-u", "root", "-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            format!("authentication failed: I'm sorry {USERNAME}. I'm afraid I can't do that")
        );
    }

    Ok(())
}

#[test]
fn double_negation() -> Result<()> {
    let env = Env([
        "Runas_Alias OP = root, operator",
        "root ALL=(ALL:ALL) NOPASSWD: ALL",
        &format!("{USERNAME} ALL = (!!OP) ALL"),
    ])
    .user(User(USERNAME).password(PASSWORD))
    .build()?;

    for user in ["root", USERNAME] {
        Command::new("sudo")
            .args(["-u", "root", "-S", "true"])
            .as_user(user)
            .stdin(PASSWORD)
            .output(&env)?
            .assert_success()?;
    }

    Ok(())
}

#[test]
fn when_specific_user_then_as_a_different_user_is_not_allowed() -> Result<()> {
    let env = Env([
        &format!("Runas_Alias OP = {USERNAME}, operator"),
        "ALL ALL = (OP) ALL",
        SUDOERS_NO_LECTURE,
    ])
    .user(User(USERNAME).password(PASSWORD))
    .user(User("ghost"))
    .build()?;

    let output = Command::new("sudo")
        .args(["-u", "ghost", "-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            format!("authentication failed: I'm sorry {USERNAME}. I'm afraid I can't do that")
        );
    }

    Ok(())
}

// Groupname
// Without the use of an alias it looks e.g. like this: "ALL ALL = (USERNAME:GROUPNAME) ALL"
// Even when 'Runas_Alias' contains both USERNAME and GROUPNAME, it depends on how the alias is referred to.
// e.g. (OP) only accepts the user, (:OP) only accepts the group and (OP:OP) accepts either user or group
// but not both together.

#[test]
fn alias_for_group() -> Result<()> {
    let env = Env([
        &format!("Runas_Alias OP = {GROUPNAME}"),
        &format!("{USERNAME} ALL = (:OP) NOPASSWD: ALL"),
    ])
    .user(User(USERNAME).password(PASSWORD))
    .user(User("otheruser"))
    .group(GROUPNAME)
    .build()?;

    Command::new("sudo")
        .args(["-g", GROUPNAME, "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn when_only_groupname_is_given_user_arg_fails() -> Result<()> {
    let env = Env([
        &format!("Runas_Alias OP = otheruser, {GROUPNAME}"),
        &format!("{USERNAME} ALL = (:OP) NOPASSWD: ALL"),
        SUDOERS_NO_LECTURE,
    ])
    .user(User(USERNAME).password(PASSWORD))
    .user(User("otheruser"))
    .group(GROUPNAME)
    .build()?;

    Command::new("sudo")
        .args(["-g", GROUPNAME, "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .args(["-u", "otheruser", "-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            format!("authentication failed: I'm sorry ferris. I'm afraid I can't do that")
        );
    }

    Ok(())
}

#[test]
fn when_only_username_is_given_group_arg_fails() -> Result<()> {
    let env = Env([
        &format!("Runas_Alias OP = otheruser, {GROUPNAME}"),
        &format!("{USERNAME} ALL = (OP) NOPASSWD: ALL"),
        SUDOERS_NO_LECTURE,
    ])
    .user(User(USERNAME).password(PASSWORD))
    .user(User("otheruser"))
    .group(GROUPNAME)
    .build()?;

    Command::new("sudo")
        .args(["-u", "otheruser", "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    let output = Command::new("sudo")
        .args(["-g", GROUPNAME, "-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(
            stderr,
            format!("authentication failed: I'm sorry ferris. I'm afraid I can't do that")
        );
    }

    Ok(())
}

#[ignore = "gh403"]
#[test]
fn user_and_group_works_when_one_is_passed_as_arg() -> Result<()> {
    let env = Env([
        &format!("Runas_Alias OP = otheruser, {GROUPNAME}"),
        &format!("{USERNAME} ALL = (OP:OP) NOPASSWD: ALL"),
    ])
    .user(User(USERNAME))
    .user(User("otheruser"))
    .group(GROUPNAME)
    .build()?;

    Command::new("sudo")
        .args(["-u", "otheruser", "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Command::new("sudo")
        .args(["-g", GROUPNAME, "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn user_and_group_succeeds_when_both_are_passed() -> Result<()> {
    if sudo_test::is_original_sudo() {
        // TODO: original sudo should pass this test after 1.9.14b2
        return Ok(());
    }

    let env = Env([
        &format!("Runas_Alias OP = otheruser, {GROUPNAME}"),
        &format!("{USERNAME} ALL = (OP:OP) NOPASSWD: ALL"),
        SUDOERS_NO_LECTURE,
    ])
    .user(User(USERNAME).password(PASSWORD))
    .user(User("otheruser"))
    .group(GROUPNAME)
    .build()?;

    Command::new("sudo")
        .args(["-u", "otheruser", "-g", GROUPNAME, "-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[ignore = "gh403"]
#[test]
fn different_aliases_user_and_group_works_when_one_is_passed_as_arg() -> Result<()> {
    let env = Env([
        &format!("Runas_Alias GROUPALIAS = {GROUPNAME}"),
        ("Runas_Alias USERALIAS = otheruser"),
        &format!("{USERNAME} ALL = (USERALIAS:GROUPALIAS) NOPASSWD: ALL"),
    ])
    .user(USERNAME)
    .user("otheruser")
    .group(GROUPNAME)
    .build()?;

    Command::new("sudo")
        .args(["-u", "otheruser", "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Command::new("sudo")
        .args(["-g", GROUPNAME, "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn different_aliases_user_and_group_succeeds_when_both_are_passed() -> Result<()> {
    if sudo_test::is_original_sudo() {
        // TODO: original sudo should pass this test after 1.9.14b2
        return Ok(());
    }

    let env = Env([
        &format!("Runas_Alias GROUPALIAS = {GROUPNAME}"),
        ("Runas_Alias USERALIAS = otheruser"),
        &format!("{USERNAME} ALL = (USERALIAS:GROUPALIAS) NOPASSWD: ALL"),
        SUDOERS_NO_LECTURE,
    ])
    .user(User(USERNAME).password(PASSWORD))
    .user(User("otheruser"))
    .group(GROUPNAME)
    .build()?;

    Command::new("sudo")
        .args(["-u", "otheruser", "-g", GROUPNAME, "-S", "true"])
        .as_user(USERNAME)
        .stdin(PASSWORD)
        .output(&env)?
        .assert_success()?;

    Ok(())
}

#[test]
fn aliases_given_on_one_line_divided_by_colon() -> Result<()> {
    let env = Env([
        "Runas_Alias GROUPALIAS = ALL : USERALIAS = ALL",
        "ALL ALL = (USERALIAS:GROUPALIAS) NOPASSWD: ALL",
    ])
    .user(USERNAME)
    .user("otheruser")
    .group("ghost")
    .build()?;

    Command::new("sudo")
        .args(["-u", "otheruser", "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Command::new("sudo")
        .args(["-g", "ghost", "true"])
        .as_user(USERNAME)
        .output(&env)?
        .assert_success()?;

    Ok(())
}
