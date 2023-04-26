#![cfg(test)]

#[macro_use]
mod macros;

mod child_process;
mod cli;
mod env_reset;
mod flag_chdir;
mod flag_group;
mod flag_login;
mod flag_shell;
mod flag_user;
mod lecture_file;
mod misc;
mod nopasswd;
mod pam;
mod pass_auth;
mod password_retry;
mod path_search;
mod perms;
mod sudoers;
mod timestamp;

mod helpers;

type Error = Box<dyn std::error::Error>;
type Result<T> = core::result::Result<T, Error>;

const USERNAME: &str = "ferris";
const GROUPNAME: &str = "rustaceans";
const PASSWORD: &str = "strong-password";
// 64 characters
const LONGEST_HOSTNAME: &str = "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijkl";

const SUDOERS_ROOT_ALL: &str = "root    ALL=(ALL:ALL) ALL";
const SUDOERS_ALL_ALL_NOPASSWD: &str = "ALL ALL=(ALL:ALL) NOPASSWD: ALL";
const SUDOERS_ROOT_ALL_NOPASSWD: &str = "root ALL=(ALL:ALL) NOPASSWD: ALL";
const SUDOERS_USER_ALL_NOPASSWD: &str = "ferris ALL=(ALL:ALL) NOPASSWD: ALL";
const SUDOERS_NO_LECTURE: &str = "Defaults	lecture=\"never\"";
const SUDOERS_NEW_LECTURE: &str = "Defaults     lecture_file = \"/etc/sudo_lecture\"";
const PAMD_SUDO_PAM_PERMIT: &str = "auth sufficient pam_permit.so";
