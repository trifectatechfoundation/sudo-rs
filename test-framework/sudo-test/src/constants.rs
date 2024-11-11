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
