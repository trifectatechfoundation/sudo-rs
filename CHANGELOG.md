# Changelog

## [0.2.12] - 2026-02-09

### Added
- Support for `-b / --background` to run commands in the background

### Changed
- visudo will now complain about unsupported command tags (#1401)

### Fixed
- When run in a pipe from ksh with `use_pty` would put the
  TTY in an unusable state (#1417, introduced in 0.2.11)
- When run as the last command in a pipe with `use_pty`, sudo-rs would not
  grant the executed command access to the TTY (#1263)
- Documentation didn't explicitly mention `-A` (#1431)

## [0.2.11] - 2025-12-16

### Added
- Support for `-A / --askpass` to enable using an external askpass program
  under control of the `SUDO_ASKPASS` environment variable.
- Functional support for localisation. This can be enabled by building sudo-rs
  with the `gettext` feature and installing a `sudo-rs.mo` in the correct
  LC_MESSAGES folder. Since we have no translations yet this is off by default.

### Changed
- sudo is always built with sudoedit functionality
- sudo no longer sets the archaic `MAIL` environment variable
- timestamps format has been changed to always check for session pid (#1132).
  As a consequence, timestamps created by earlier versions of sudo-rs are
  invalidated after upgrading to this version.
- The folder containing zoneinfo is detected at runtime; `build.rs` was removed
- The default value of `Defaults editor` has been changed on Linux to remove the
  dependence on Debian-specific `/usr/bin/editor`.

### Fixed
- `sudo -i` made `root` as owner of the pty instead of the login user (#1333)
- visudo usability improvements (#1388, #1394)
- Mistakes in the man pages (#1338, #1362, #1387)
- Better error message when /etc/sudoers contains regular expressions (#1352)
- Better error message when /etc/sudoers is missing (#1368)
- Redirecting input/output to another TTY was not recognised as redirection;
  this fix originated from Todd Miller's sudo (#1380)

## [0.2.10] - 2025-11-10

### Changed
- Message shown at password timeout has been made nicer (#1171)
- Interrupting a PAM module that handles its own input such as `fprintd`
  will not prevent other authentication modes from being tried. (#1308)

### Fixed
- Some formatting mistakes in the man pages (#1285)
- Unhandled exception if user would attempt to execute a folder (#1298)
- `Defaults passwd_tries=0` effectively disabled the limit on password tries
  rather than setting the limit to zero (#1313)
- sudoedit ignored supplementary groups when checking writability (#1321)
- A partially typed password would be output to standard input if a timeout
  occurred when `Defaults pwfeedback` was not enabled (GHSA-q428-6v73-fc4q).
- Timestamp files did not take into account the setting of the
 `Defaults targetpw` and `Defaults rootpw` (GHSA-c978-wq47-pvvw)
- Minor tokenization error in /etc/sudoers processing (#1296)

## [0.2.9] - 2025-10-03

### Added
- `SUDO_HOME` variable will now be set to the invoking user's home directory

### Changed
- `Defaults noninteractive_auth` now controls whether PAM authentication
  modules are allowed to attempt authentication when `sudo --non-interactive` is
  being used (new default: off). Previous versions had this as always-on
  to allow fully automatic authentication methods to succeed.

### Fixed
- Two bugs in managing the PTY connected to the child process that negatively
  impacted interactive usability (#1130, #1264)
- `visudo --help` showed command flags that were removed (#1239)
- Format flags in `SUDO_PROMPT` were not expanded (#1252)
- `sudo` would abort with an unhandled exception instead if an attempt was
  was made to match a "netgroup", instead of ignoring these (#1262)
- A few tokenizer errors in /etc/sudoers processing (#1273, #1274, #1283)
- Some formatting mistakes in the man pages (#1285)

## [0.2.8] - 2025-08-04

### Added
- `sudo -e`, `sudoedit` to safely edit files as another user.

### Fixed
- `NOEXEC:` could not be used to prevent all shell escapes on multi-architecture
  installations (#1229)
- `sudo --list` would not show `NOEXEC`, `SETENV` and `APPARMOR_PROFILE` (#1228)
- Skip paths not accessible by the target user during command resolution (#1234)

## [0.2.7] - 2025-07-01

### Added
- Linux kernels older than 5.9 are now supported.
- Support for `Defaults noexec`/`NOEXEC:` on Linux systems based on seccomp
  filtering to prevent shell escapes in wide range of cases. This should also
  work on programs not written in C and statically linked executables.
- Support for `passwd_timeout`
- Support for `umask` and `umask_override`
- `--preserve-env=VAR` is now supported to preserve selected environment
  variables in a more convenient way

### Changed
- sudo-rs now uses CLOEXEC to close open file descriptors in the child process
- Relative paths like `./` in `secure_path`/`PATH` are now ignored.
- `apparmor.so` is dynamically loaded by sudo itself, as-needed

### Fixed
- Usernames that start with `_` or have non-western characters were not supported
  as a valid username in /etc/sudoers (#1149)
- Other usability improvements in /etc/sudoers (#1117, #1126, #1134, #1157)

## [0.2.6] - 2025-05-06

### Added
- Support for `Defaults setenv`
- Support for the `list` pseudocommand to control `sudo -U`
- Support for switching AppArmor profiles though `Defaults apparmor_profile` and
  the `APPARMOR_PROFILE` command modifier. To enable this, build sudo-rs with
  the apparmor feature enabled.

### Changed
- Added a check against PAM modules changing the user during authentication (#1062)
- `list` pseudocommand now controls whether a password is required for `sudo -l -U`

### Fixed
- Usernames commonly used by Active Directory were not parsed correctly (#1064)
- Test compilation was broken on 32-bit systems (#1074)
- `pwfeedback` was ignored for `sudo --list` and `sudo --validate`  (#1092)
-  Compilation with musl instead of glibc on Linux was not possible (#1084)
- `sudo --list` now does more checking before reporting errors or listing the
  rights of a user, fixing two security bugs (CVE-2025-46717 and CVE-2025-46718)

## [0.2.5] - 2025-04-01

### Added
- `sudo visudo` will protect you from accidentally locking yourself out
- Support for `--prompt` and `SUDO_PROMPT` environment variable
- Support for `Defaults targetpw`
- Support for `VAR=VALUE` matching in `Defaults env_keep/env_check`
- Support for `--bell`

### Changed
- Portability: sudo-rs supports FreeBSD!
- `sudo -v` will only ask for a password if the policy requires it

### Fixed
- Manual wrongly claimed `timestamp_timeout` supported negative values (#1032)
- `timestamp_timeout` in excess of 292 billion years were not rejected (#1048)
- Usernames in /etc/sudoers can contain special characters by using double
  quotes or escaping them (#1045)

## [0.2.4] - 2025-02-25

### Added
- Support for `SETENV:` and corresponding `sudo VAR=value command` syntax
- Support for `Defaults rootpw`
- Support for `Defaults pwfeedback`
- Support for host/user/runas/command-specific `Defaults`

### Changed
- Portability: sudo-rs now has experimental support for FreeBSD!
- `pam-login` feature now controls if PAM service name 'sudo-i' is used

### Fixed
- Bug in syslog writer could cause sudo to hang (#856)
- SHELL was not canonicalized when using `sudo -s` or `sudo -i` (#962)
- RunAs_Spec was not carried over on the same /etc/sudoers line (#974)
- sudo --list did not unfold multiple-level aliases (#978)
- The man page for sudoers was missing (#943)

### Other
- sudo-rs copyright changed to Trifecta Tech Foundation

## [0.2.3] - 2024-07-11

### Changed
- Portability: sudo-rs now is compatible with s390x-unknown-linux-gnu
- Removed unneeded code & fix hints given by newer Rust version

### Fixed
- `visudo` would not properly truncate a `sudoers` file
- high CPU load when child process did not terminate after closure of a terminal

## [0.2.2] - 2024-02-02

### Changed
- Several changes to the code to improve type safety
- Improved error message when a PTY cannot be opened
- Improved portability of the PAM bindings
- su: improved parsing of su command line options
- Add path information to parse errors originating from included files

### Fixed
- Fixed a panic with large messages written to the syslog
- sudo: respect `--login` regardless of the presence of `--chdir`

## [0.2.1] - 2023-09-21

### Changed
- Session records/timestamps are now stored in files with uids instead of
  usernames, fixing a security bug (CVE-2023-42456)
- `visudo` will now resolve `EDITOR` via `PATH`
- Input/output errors while writing text to the terminal no longer cause sudo to
  exit immediately
- Switched several internal API calls from libc to Rust's std library
- The `%h` escape sequence in sudoers includes directives is not supported in
  sudo-rs, this now gives a better diagnostic and no longer tries to include the
  file
- Our PAM integration was hardened against allocation failures
- An attempt was made to harden against rowhammer type attacks
- Release builds no longer include debugging symbols

### Fixed
- Fixed an invalid parsing when an escaped null byte was present in the sudoers
  file
- Replaced informal error message in `visudo` with a proper error message


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

[0.2.3]: https://github.com/trifectatechfoundation/sudo-rs/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/trifectatechfoundation/sudo-rs/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/trifectatechfoundation/sudo-rs/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/trifectatechfoundation/sudo-rs/compare/v0.2.0-dev.20230711...v0.2.0
[0.2.0-dev.20230711]: https://github.com/trifectatechfoundation/sudo-rs/compare/v0.2.0-dev.20230703...v0.2.0-dev.20230711
[0.2.0-dev.20230703]: https://github.com/trifectatechfoundation/sudo-rs/compare/v0.2.0-dev.20230627...v0.2.0-dev.20230703
[0.2.0-dev.20230627]: https://github.com/trifectatechfoundation/sudo-rs/compare/v0.1.0-dev.20230620...v0.2.0-dev.20230627
