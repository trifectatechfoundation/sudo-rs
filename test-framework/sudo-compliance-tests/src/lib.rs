#![cfg(test)]

use core::fmt;

#[macro_use]
mod macros;

mod child_process;
mod cli;
mod env_reset;
mod flag_chdir;
mod flag_group;
mod flag_login;
mod flag_non_interactive;
mod flag_shell;
mod flag_user;
mod lecture;
mod lecture_file;
mod misc;
mod nopasswd;
mod pam;
mod pass_auth;
mod password_retry;
mod path_search;
mod perms;
mod sudo_ps1;
mod sudoers;
mod syslog;
mod timestamp;
mod use_pty;

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
const SUDOERS_USER_ALL_ALL: &str = "ferris ALL=(ALL:ALL) ALL";
const SUDOERS_NO_LECTURE: &str = "Defaults lecture=\"never\"";
const SUDOERS_ONCE_LECTURE: &str = "Defaults lecture=\"once\"";
const SUDOERS_ALWAYS_LECTURE: &str = "Defaults lecture=\"always\"";
const SUDOERS_NEW_LECTURE: &str = "Defaults lecture_file = \"/etc/sudo_lecture\"";
const SUDOERS_NEW_LECTURE_USER: &str = "Defaults:ferris lecture_file = \"/etc/sudo_lecture\"";
const PAMD_SUDO_PAM_PERMIT: &str = "auth sufficient pam_permit.so";

const OG_SUDO_STANDARD_LECTURE: &str= "\nWe trust you have received the usual lecture from the local System\nAdministrator. It usually boils down to these three things:\n\n    #1) Respect the privacy of others.\n    #2) Think before you type.\n    #3) With great power comes great responsibility.";

const SUDO_RS_IS_UNSTABLE: &str =
    "SUDO_RS_IS_UNSTABLE=I accept that my system may break unexpectedly";

const SUDO_ENV_DEFAULT_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";
const SUDO_ENV_DEFAULT_TERM: &str = "unknown";

const SUDOERS_USE_PTY: &str = "Defaults use_pty";

enum EnvList {
    Check,
    Keep,
}

impl fmt::Display for EnvList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EnvList::Check => "env_check",
            EnvList::Keep => "env_keep",
        };
        f.write_str(s)
    }
}
