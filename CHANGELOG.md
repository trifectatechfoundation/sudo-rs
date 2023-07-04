# Changelog

## [Unreleased]

### Changed
- The SUDO_RS_IS_UNSTABLE environment variable is no longer required
- Sudo-rs will now read `/etc/sudoers-rs` or `/etc/sudoers` if the former is
  not available. We no longer read `/etc/sudoers.test`

### Fixed
- Only call ttyname and isatty on character devices

## [0.2.0-dev.20230703] - 2023-07-03

### Added
- Add `timestamp_timeout` support in sudoers file
- Add ability to disable `use_pty` in the sudoers file

### Changed
- Set the TTY name for PAM sessions on a TTY
- Set the requesting user for PAM sessions
- Simplified some error messages when a command could not be executed
- Reveal less about what caused a command not to be executable
- Continued rework of the pty exec

### Fixed
- Fixed exit codes for `su`
- Fixed environment filtering for `su`
- Fixed `SHELL` handling for `su`

## [0.2.0-dev.20230627] - 2023-06-27

### Added
- Add `passwd_tries` support in sudoers file
- Add developer logs (only enabled with the `dev` feature)

### Changed
- Only use a PTY to spawn the process if a TTY is available
- Continued rework of the pty exec
- Aliasing is now implemented similarly to the original sudo
- You can no longer define an `ALL` alias in the sudoers file
- Use canonicalized paths for the executed binaries
- Simplified CLI help to only display supported actions

[Unreleased]: https://github.com/memorysafety/sudo-rs/compare/v0.2.0-dev.20230703...HEAD
[0.2.0-dev.20230703]: https://github.com/memorysafety/sudo-rs/compare/v0.2.0-dev.20230627...v0.2.0-dev.20230703
[0.2.0-dev.20230627]: https://github.com/memorysafety/sudo-rs/compare/v0.1.0-dev.20230620...v0.2.0-dev.20230627
