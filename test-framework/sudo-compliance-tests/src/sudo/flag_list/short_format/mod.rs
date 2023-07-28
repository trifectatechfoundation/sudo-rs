use sudo_test::{Command, Env};

use crate::{Result, HOSTNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!($($tt)*)
        })

    };
}

// NOTE all the input sudoers files have extra whitespaces to check that `--list` pretty prints the
// sudoers entries

fn sudo_list_of(sudoers: &str) -> Result<String> {
    let env = Env(sudoers).hostname(HOSTNAME).build()?;
    Command::new("sudo").arg("-l").output(&env)?.stdout()
}

#[test]
fn no_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL = ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn empty_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = ( )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = ( ferris )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_id_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = ( #0 )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_group_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = ( %root )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_group_id_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = ( %#0 )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_non_unix_group_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = ( %:root )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_non_unix_group_id_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = ( %:#0 )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn not_user_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = ( ! ferris )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn multiple_users_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = ( ferris ,  root )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn group_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = (  :  crabs )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn not_group_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = (  : ! crabs )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn multiple_group_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = (  :  crabs ,  root )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn complex_runas() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = ( ! ferris ,  root  :  crabs ,  !root )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
#[ignore = "gh718"]
fn command_alias() -> Result<()> {
    let stdout = sudo_list_of(
        "Cmnd_Alias COMMANDS = /usr/bin/true, /usr/bin/false
 ALL  ALL  = /usr/bin/ls, COMMANDS ",
    )?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn command_arguments() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = /usr/bin/true  a  b  c  ,  /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn multiple_commands() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = /usr/bin/true ,  /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn multiple_runas_groups() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = ( root ) /usr/bin/true ,  ( ferris ) /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn implicit_runas_group() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = /usr/bin/true , ( ferris ) /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_any() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = CWD = * /usr/bin/true ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_path() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = CWD = /home /usr/bin/true ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_multiple_commands() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = CWD = * /usr/bin/true ,  /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_multiple_runas_groups() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = CWD = * /usr/bin/true ,  ( ferris ) /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_override() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = CWD = * /usr/bin/true , CWD = /home /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_not_in_first_position() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = /usr/bin/true , CWD = * /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_across_runas_groups() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = CWD = * /usr/bin/true , (ferris) /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_override_across_runas_groups() -> Result<()> {
    let stdout = sudo_list_of(
        " ALL  ALL  = CWD = * /usr/bin/true , (ferris) /usr/bin/false , CWD = /home /usr/bin/ls ",
    )?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn passwd() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = PASSWD : /usr/bin/true ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn nopasswd() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = NOPASSWD : /usr/bin/true ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn passwd_nopasswd_override() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = PASSWD : /usr/bin/true , NOPASSWD: /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn nopasswd_passwd_override() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = NOPASSWD : /usr/bin/true , PASSWD: /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn nopasswd_passwd_on_same_command() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = NOPASSWD : PASSWD : /usr/bin/true ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn nopasswd_across_runas_groups() -> Result<()> {
    let stdout =
        sudo_list_of(" ALL  ALL  = NOPASSWD : /usr/bin/true , ( ferris ) /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn passwd_across_runas_groups() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = PASSWD : /usr/bin/true , ( ferris ) /usr/bin/false ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn nopasswd_passwd_override_across_runas_groups() -> Result<()> {
    let stdout = sudo_list_of(
        " ALL  ALL  = NOPASSWD : /usr/bin/true , ( ferris ) /usr/bin/false , PASSWD : /usr/bin/ls ",
    )?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_nopasswd() -> Result<()> {
    let stdout = sudo_list_of(" ALL  ALL  = CWD = * NOPASSWD : /usr/bin/true ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn multiple_lines() -> Result<()> {
    let stdout = sudo_list_of(
        " ALL  ALL  = /usr/bin/true , /usr/bin/false
 root ALL = /usr/bin/ls ",
    )?;
    assert_snapshot!(stdout);
    Ok(())
}
