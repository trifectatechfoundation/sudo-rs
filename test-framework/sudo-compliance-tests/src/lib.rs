#![cfg(test)]

#[macro_use]
mod macros;

mod child_process;
mod env_reset;
mod flag_user;
mod nopasswd;
mod pass_auth;
mod perms;
mod sudoers;

type Error = Box<dyn std::error::Error>;
type Result<T> = core::result::Result<T, Error>;

const USERNAME: &str = "ferris";
const GROUPNAME: &str = "rustaceans";
const PASSWORD: &str = "strong-password";

const SUDOERS_ROOT_ALL: &str = "root    ALL=(ALL:ALL) ALL";
const SUDOERS_ROOT_ALL_NOPASSWD: &str = "root ALL=(ALL:ALL) NOPASSWD: ALL";
const SUDOERS_USER_ALL_NOPASSWD: &str = "ferris ALL=(ALL:ALL) NOPASSWD: ALL";
