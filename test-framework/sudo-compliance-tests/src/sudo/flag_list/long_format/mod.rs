use sudo_test::{Command, Env, BIN_FALSE, BIN_LS, BIN_TRUE, ETC_SUDOERS};

use crate::{Result, HOSTNAME};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![
                (BIN_LS, "<BIN_LS>"),
                (&format!("Sudoers entry: {ETC_SUDOERS}"), "Sudoers entry:"),
            ],
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!($($tt)*)
        })

    };
}

// NOTE all the input sudoers files have extra whitespaces to check that `--list` pretty prints the
// sudoers entries

fn sudo_ll_of(sudoers: &str) -> Result<String> {
    let env = Env(sudoers).hostname(HOSTNAME).build()?;
    Command::new("sudo")
        .args(["-l", "-l"])
        .output(&env)?
        .stdout()
}

#[test]
fn no_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL = ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn empty_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = ( )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = ( ferris )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_id_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = ( #0 )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_group_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = ( %root )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_group_id_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = ( %#0 )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_non_unix_group_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = ( %:root )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn user_non_unix_group_id_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = ( %:#0 )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn not_user_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = ( ! ferris )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn multiple_users_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = ( ferris ,  root )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn group_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = (  :  crabs )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn not_group_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = (  : ! crabs )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn multiple_group_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = (  :  crabs ,  root )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn complex_runas() -> Result<()> {
    let stdout = sudo_ll_of(" ALL  ALL  = ( ! ferris ,  root  :  crabs ,  !root )  ALL ")?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn command_alias() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        "Cmnd_Alias COMMANDS = {BIN_TRUE}, {BIN_FALSE}
 ALL  ALL  = {BIN_LS}, COMMANDS "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn negated_command_alias() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        "Cmnd_Alias COMMANDS = {BIN_TRUE}, !{BIN_FALSE}
                 ALL  ALL  = {BIN_LS}, !COMMANDS "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn command_arguments() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = {BIN_TRUE}  a  b  c  ,  {BIN_FALSE} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn multiple_commands() -> Result<()> {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = {BIN_TRUE} ,  {BIN_FALSE} "))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn multiple_runas_groups() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = ( root ) {BIN_TRUE} ,  ( ferris ) {BIN_FALSE} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn implicit_runas_group() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = {BIN_TRUE} , ( ferris ) {BIN_FALSE} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_any() -> Result<()> {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = CWD = * {BIN_TRUE} "))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_path() -> Result<()> {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = CWD = /home {BIN_TRUE} "))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_multiple_commands() -> Result<()> {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = CWD = * {BIN_TRUE} ,  {BIN_FALSE} "))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_multiple_runas_groups() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = CWD = * {BIN_TRUE} ,  ( ferris ) {BIN_FALSE} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_override() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = CWD = * {BIN_TRUE} , CWD = /home {BIN_FALSE} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_not_in_first_position() -> Result<()> {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = {BIN_TRUE} , CWD = * {BIN_FALSE} "))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_across_runas_groups() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = CWD = * {BIN_TRUE} , (ferris) {BIN_FALSE} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_override_across_runas_groups() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = CWD = * {BIN_TRUE} , (ferris) {BIN_FALSE} , CWD = /home {BIN_LS} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn passwd() -> Result<()> {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = PASSWD : {BIN_TRUE} "))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn nopasswd() -> Result<()> {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = NOPASSWD : {BIN_TRUE} "))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn passwd_nopasswd_override() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = PASSWD : {BIN_TRUE} , NOPASSWD: {BIN_FALSE} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn nopasswd_passwd_override() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = NOPASSWD : {BIN_TRUE} , PASSWD: {BIN_FALSE} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn nopasswd_passwd_on_same_command() -> Result<()> {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = NOPASSWD : PASSWD : {BIN_TRUE} "))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn nopasswd_across_runas_groups() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = NOPASSWD : {BIN_TRUE} , ( ferris ) {BIN_FALSE} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn passwd_across_runas_groups() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = PASSWD : {BIN_TRUE} , ( ferris ) {BIN_FALSE} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn nopasswd_passwd_override_across_runas_groups() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = NOPASSWD : {BIN_TRUE} , ( ferris ) {BIN_FALSE} , PASSWD : {BIN_LS} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn cwd_nopasswd() -> Result<()> {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = CWD = * NOPASSWD : {BIN_TRUE} "))?;
    assert_snapshot!(stdout);
    Ok(())
}

#[test]
fn multiple_lines() -> Result<()> {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = {BIN_TRUE} , {BIN_FALSE}
 root ALL = {BIN_LS} "
    ))?;
    assert_snapshot!(stdout);
    Ok(())
}
