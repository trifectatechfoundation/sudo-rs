# Changelog

## [0.2.0] - 2023-08-29

### Added
- `visudo` can set/fix file permissions using the `--perms` CLI flag
- `visudo` can set/fix the file owner using the `--owner` CLI flag
- Read `env_editor` from sudoers file for visudo
- Add basic support for `--list` in sudo

### Changed
- `visudo` now uses a random filename for the temporary file you are editing
- `su` now runs with a PTY by default
- Included files with relative paths in the sudoers file are imported relative
  from the sudoers file
- `sudo` now checks if ownership and setuid bits have been set correctly on
  its binary
- When syslog messages are too large they will be split between multiple
  messages to prevent message truncation
- We now accept a wider range of dependencies
- Our MSRV (minimum supported rust version) has been set at 1.70.0

### Fixed
- Set arg0 to the non-resolved filename when running a command, preventing
  issues with symlinks when commands rely on link filenames

## [0.2.0-dev.20230711] - 2023-07-11

### Added
- Add initial `visudo` implementation
- Add support for `~` in `--chdir`
- Log commands that will be executed in the auth syslog
- Add a manpage for the `sudo` command

### Changed
- The SUDO_RS_IS_UNSTABLE environment variable is no longer required
- Sudo-rs will now read `/etc/sudoers-rs` or `/etc/sudoers` if the former is
  not available. We no longer read `/etc/sudoers.test`
- Removed signal-hook and signal-hook-registry dependencies
- Improved error handling when `--chdir` is passed but not allowed
- Properly handle `SIGWINCH` when running commands with a PTY

### Fixed
- Only call ttyname and isatty on character devices
- Fixed a bug in syslog FFI

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

[0.2.0]: https://github.com/memorysafety/sudo-rs/compare/v0.2.0-dev.20230711...v0.2.0
[0.2.0-dev.20230711]: https://github.com/memorysafety/sudo-rs/compare/v0.2.0-dev.20230703...v0.2.0-dev.20230711
[0.2.0-dev.20230703]: https://github.com/memorysafety/sudo-rs/compare/v0.2.0-dev.20230627...v0.2.0-dev.20230703
[0.2.0-dev.20230627]: https://github.com/memorysafety/sudo-rs/compare/v0.1.0-dev.20230620...v0.2.0-dev.20230627
