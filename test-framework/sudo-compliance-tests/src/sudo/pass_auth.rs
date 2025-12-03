//! Scenarios where password authentication is needed

// NOTE all these tests assume that the invoking user passes the sudoers file 'User_List' criteria

mod askpass;
mod stdin;
mod tty;

#[cfg(not(target_os = "freebsd"))]
const MAX_PASSWORD_SIZE: usize = 511; // MAX_PAM_RESPONSE_SIZE - 1 null byte
#[cfg(target_os = "freebsd")]
const MAX_PASSWORD_SIZE: usize = 128;
