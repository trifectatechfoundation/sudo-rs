/// The parent of the directory where sudo will look for the sudoers file.
#[cfg(not(target_os = "freebsd"))]
pub const ETC_PARENT_DIR: &str = "/";

/// The parent of the directory where sudo will look for the sudoers file.
#[cfg(target_os = "freebsd")]
pub const ETC_PARENT_DIR: &str = "/usr/local/";

/// The directory where sudo will look for the sudoers file.
#[cfg(not(target_os = "freebsd"))]
pub const ETC_DIR: &str = "/etc";

/// The directory where sudo will look for the sudoers file.
#[cfg(target_os = "freebsd")]
pub const ETC_DIR: &str = "/usr/local/etc";

/// The path where sudo will look for the sudoers file.
#[cfg(not(target_os = "freebsd"))]
pub const ETC_SUDOERS: &str = "/etc/sudoers";

/// The path where sudo will look for the sudoers file.
#[cfg(target_os = "freebsd")]
pub const ETC_SUDOERS: &str = "/usr/local/etc/sudoers";

/// The name of the primary group of the root user.
#[cfg(not(target_os = "freebsd"))]
pub const ROOT_GROUP: &str = "root";

/// The name of the primary group of the root user.
#[cfg(target_os = "freebsd")]
pub const ROOT_GROUP: &str = "wheel";

/// The path to the `sudo` binary.
#[cfg(not(target_os = "freebsd"))]
pub const BIN_SUDO: &str = "/usr/bin/sudo";

/// The path to the `sudo` binary.
#[cfg(target_os = "freebsd")]
pub const BIN_SUDO: &str = "/usr/local/bin/sudo";

/// The path to the `ls` binary.
#[cfg(not(target_os = "freebsd"))]
pub const BIN_LS: &str = "/usr/bin/ls";

/// The path to the `ls` binary.
#[cfg(target_os = "freebsd")]
pub const BIN_LS: &str = "/bin/ls";

/// The path to the `pwd` binary.
#[cfg(not(target_os = "freebsd"))]
pub const BIN_PWD: &str = "/usr/bin/pwd";

/// The path to the `pwd` binary.
#[cfg(target_os = "freebsd")]
pub const BIN_PWD: &str = "/bin/pwd";

/// The path to the `true` binary.
pub const BIN_TRUE: &str = "/usr/bin/true";

/// The path to the `false` binary.
pub const BIN_FALSE: &str = "/usr/bin/false";

/// The path to the `bash` binary.
#[cfg(not(target_os = "freebsd"))]
pub const BIN_BASH: &str = "/usr/bin/bash";

/// The path to the `bash` binary.
#[cfg(target_os = "freebsd")]
pub const BIN_BASH: &str = "/usr/local/bin/bash";
