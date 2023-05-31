//! Scenarios where password authentication is needed

// NOTE all these tests assume that the invoking user passes the sudoers file 'User_List' criteria

mod stdin;
mod tty;

const MAX_PAM_RESPONSE_SIZE: usize = 512;
