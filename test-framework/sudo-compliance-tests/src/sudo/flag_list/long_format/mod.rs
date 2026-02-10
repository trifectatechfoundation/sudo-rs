use sudo_test::{BIN_FALSE, BIN_LS, BIN_TRUE, Command, ETC_SUDOERS, Env};

use crate::HOSTNAME;

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            filters => vec![
                (BIN_LS, "<BIN_LS>"),
                (&format!("Sudoers entry: {ETC_SUDOERS}"), "Sudoers entry:"),
                ("Matching Defaults entries for ferruccio on container:
    !fqdn, !lecture, !mailerpath
", "")
            ],
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!($($tt)*)
        })

    };
}

// NOTE all the input sudoers files have extra whitespaces to check that `--list` pretty prints the
// sudoers entries

fn sudo_ll_of(sudoers: &str) -> String {
    let user = "ferruccio";
    let sudoers = ["ALL ALL = NOPASSWD: /tmp", sudoers].join("\n");
    let env = Env(sudoers).hostname(HOSTNAME).user(user).build();
    Command::new("sudo")
        .as_user(user)
        .args(["-l", "-l"])
        .output(&env)
        .stdout()
}

#[test]
fn no_runas() {
    let stdout = sudo_ll_of(" ALL  ALL = ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn empty_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = ( )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn user_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = ( ferris )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn user_id_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = ( #0 )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn user_group_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = ( %root )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn user_group_id_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = ( %#0 )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn user_non_unix_group_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = ( %:root )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn user_non_unix_group_id_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = ( %:#0 )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn not_user_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = ( ! ferris )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn multiple_users_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = ( ferris ,  root )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn group_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = (  :  crabs )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn not_group_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = (  : ! crabs )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn multiple_group_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = (  :  crabs ,  root )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn complex_runas() {
    let stdout = sudo_ll_of(" ALL  ALL  = ( ! ferris ,  root  :  crabs ,  !root )  ALL ");
    assert_snapshot!(stdout);
}

#[test]
fn command_alias() {
    let stdout = sudo_ll_of(&format!(
        "Cmnd_Alias COMMANDS = {BIN_TRUE}, {BIN_FALSE}
 ALL  ALL  = {BIN_LS}, COMMANDS "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn negated_command_alias() {
    let stdout = sudo_ll_of(&format!(
        "Cmnd_Alias COMMANDS = {BIN_TRUE}, !{BIN_FALSE}
                 ALL  ALL  = {BIN_LS}, !COMMANDS "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn command_arguments() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = {BIN_TRUE}  a  b  c  ,  {BIN_FALSE} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn multiple_commands() {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = {BIN_TRUE} ,  {BIN_FALSE} "));
    assert_snapshot!(stdout);
}

#[test]
fn multiple_runas_groups() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = ( root ) {BIN_TRUE} ,  ( ferris ) {BIN_FALSE} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn implicit_runas_group() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = {BIN_TRUE} , ( ferris ) {BIN_FALSE} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn cwd_any() {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = CWD = * {BIN_TRUE} "));
    assert_snapshot!(stdout);
}

#[test]
fn cwd_path() {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = CWD = /home {BIN_TRUE} "));
    assert_snapshot!(stdout);
}

#[test]
fn cwd_multiple_commands() {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = CWD = * {BIN_TRUE} ,  {BIN_FALSE} "));
    assert_snapshot!(stdout);
}

#[test]
fn cwd_multiple_runas_groups() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = CWD = * {BIN_TRUE} ,  ( ferris ) {BIN_FALSE} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn cwd_override() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = CWD = * {BIN_TRUE} , CWD = /home {BIN_FALSE} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn cwd_not_in_first_position() {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = {BIN_TRUE} , CWD = * {BIN_FALSE} "));
    assert_snapshot!(stdout);
}

#[test]
fn cwd_across_runas_groups() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = CWD = * {BIN_TRUE} , (ferris) {BIN_FALSE} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn cwd_override_across_runas_groups() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = CWD = * {BIN_TRUE} , (ferris) {BIN_FALSE} , CWD = /home {BIN_LS} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn passwd() {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = PASSWD : {BIN_TRUE} "));
    assert_snapshot!(stdout);
}

#[test]
fn nopasswd() {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = NOPASSWD : {BIN_TRUE} "));
    assert_snapshot!(stdout);
}

#[test]
fn passwd_nopasswd_override() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = PASSWD : {BIN_TRUE} , NOPASSWD: {BIN_FALSE} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn nopasswd_passwd_override() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = NOPASSWD : {BIN_TRUE} , PASSWD: {BIN_FALSE} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn nopasswd_passwd_on_same_command() {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = NOPASSWD : PASSWD : {BIN_TRUE} "));
    assert_snapshot!(stdout);
}

#[test]
fn nopasswd_across_runas_groups() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = NOPASSWD : {BIN_TRUE} , ( ferris ) {BIN_FALSE} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn passwd_across_runas_groups() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = PASSWD : {BIN_TRUE} , ( ferris ) {BIN_FALSE} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn nopasswd_passwd_override_across_runas_groups() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = NOPASSWD : {BIN_TRUE} , ( ferris ) {BIN_FALSE} , PASSWD : {BIN_LS} "
    ));
    assert_snapshot!(stdout);
}

#[test]
fn cwd_nopasswd() {
    let stdout = sudo_ll_of(&format!(" ALL  ALL  = CWD = * NOPASSWD : {BIN_TRUE} "));
    assert_snapshot!(stdout);
}

#[test]
fn multiple_lines() {
    let stdout = sudo_ll_of(&format!(
        " ALL  ALL  = {BIN_TRUE} , {BIN_FALSE}
 root ALL = {BIN_LS} "
    ));
    assert_snapshot!(stdout);
}
