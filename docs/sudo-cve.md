# Past sudo CVEs

This listing contains security issues originally found in sudo but which could
also be relevant for sudo-rs.

## Possibly relevant CVEs / advisories

These CVEs/advisories are possibly relevant to sudo-rs:

| CVE                    | Tests | Sudo Advisory / Attack notes                                                |
| ---------------------- | ----- | --------------------------------------------------------------------------- |
| CVE-1999-0958 [^1]     |       | Relative path attack (.. attack)                                            |
| CVE-1999-1496 [^2]     |       | Information leakage on which commands exist                                 |
| - [^rust]              |       | https://www.sudo.ws/security/advisories/heap_corruption/                    |
| CVE-2002-0184 [^rust]  |       | https://www.sudo.ws/security/advisories/prompt/                             |
| CVE-2004-1051 [^4]     |       | https://www.sudo.ws/security/advisories/bash_functions/                     |
| CVE-2004-1689 [^22]    |       | https://www.sudo.ws/security/advisories/sudoedit/                           |
| CVE-2005-1119 [^5]     |       | Corrupt arbitrary files via a symlink attack                                |
| CVE-2005-1993 [^6]     |       | https://www.sudo.ws/security/advisories/path_race/                          |
| CVE-2005-4890 [^7]     |       | TTY hijacking when a privileged user uses sudo to run unprivileged commands |
| - [^9]                 |       | https://www.sudo.ws/security/advisories/cmnd_alias_negation/                |
| CVE-2010-0426 [^23]    |       | https://www.sudo.ws/security/advisories/sudoedit_escalate/                  |
| CVE-2010-1163 [^23]    |       | https://www.sudo.ws/security/advisories/sudoedit_escalate2/                 |
| CVE-2010-1646 [^10]    |       | https://www.sudo.ws/security/advisories/secure_path/                        |
| CVE-2010-2956 [^11]    |       | https://www.sudo.ws/security/advisories/runas_group/                        |
| CVE-2011-0010 [^12]    |       | https://www.sudo.ws/security/advisories/runas_group_pw/                     |
| CVE-2012-0809 [^13]    |       | https://www.sudo.ws/security/advisories/sudo_debug/                         |
| CVE-2013-1775 [^14]    |       | https://www.sudo.ws/security/advisories/epoch_ticket/                       |
| CVE-2013-1776 [^15]    |       | https://www.sudo.ws/security/advisories/tty_tickets/                        |
| CVE-2013-2776 [^15]    |       | https://www.sudo.ws/security/advisories/tty_tickets/                        |
| CVE-2013-2777 [^15]    |       | https://www.sudo.ws/security/advisories/tty_tickets/                        |
| CVE-2014-9680 [^16]    |       | https://www.sudo.ws/security/advisories/tz/                                 |
| CVE-2015-5602 [^24]    |       | https://bugzilla.sudo.ws/show_bug.cgi?id=707                                |
| CVE-2016-7032 [^17]    |       | https://www.sudo.ws/security/advisories/noexec_bypass/                      |
| CVE-2016-7076 [^17]    |       | https://www.sudo.ws/security/advisories/noexec_wordexp/                     |
| CVE-2017-1000367 [^18] |       | https://www.sudo.ws/security/advisories/linux_tty/                          |
| CVE-2017-1000368 [^18] |       | https://www.sudo.ws/security/advisories/linux_tty/                          |
| CVE-2019-18634 [^rust] |       | https://www.sudo.ws/security/advisories/pwfeedback/                         |
| CVE-2021-3156  [^21]   |       | https://www.sudo.ws/security/advisories/unescape_overflow/                  |
| CVE-2021-23239 [^25]   |       | https://www.sudo.ws/releases/stable/#1.9.5                                  |
| CVE-2023-22809 [^20]   |       | https://www.sudo.ws/security/advisories/sudoedit_any/                       |
| CVE-2023-28486 [^19]   |       | Syslog messages do not escape control characters                            |

[^1]: All our path checks should only ever be done with absolute paths
[^2]: We try to take care to only expose relevant information to the user
[^rust]: Our usage of Rust should mostly prevent heap corruption bugs from occurring
[^4]: env_reset is always enabled in sudo-rs, additionally we apply filtering to
      several variables to prevent any additional attack paths
[^5]: -
[^6]: Sudo-rs uses the suggested realpath function, as it is considered available
      enough for our target systems
[^7]: To prevent attacks, a PTY must be used when running commands within a TTY,
      which is enabled by default in sudo-rs
[^9]: -
[^10]: -
[^11]: -
[^12]: -
[^13]: -
[^14]: -
[^15]: -
[^16]: -
[^17]: Sudo-rs uses seccomp filtering rather than libc function interception through LD_PRELOAD.
[^18]: -
[^19]: -
[^20]: Sudo-rs doesn't use a "stringly typed" interface between the execution and policy modules.
[^21]: Rust memory safety should prevent this, sudo-rs doesn't allow `-s` and `-e` to be combined, and sudo-rs
       doesn't "unescape" program arguments in the sudoers module
[^22]: Reading the changed temporary file back is done by an unprivileged helper process.
[^23]: Sudo-rs matches commands based on (canonicalized and absolute) path names, so `sudoedit` never matches; furthermore,
       invoking `sudo /path/to/sudoedit` will instead run `sudoedit` as the current user.
[^24]: Sudo-rs doesn't allow wildcards or symlinks in configuration arguments to sudoedit, and checks that all path
       components are not writable by the calling user.
[^25]: Sudo-rs opens all components of the path to be edited exactly once, and checks that all path components are
       not writable by the calling user.

## Non-applicable CVEs

These CVEs are almost entirely not applicable in the current sudo-rs codebase,
mainly because the feature they relate to is not implemented. Sometimes this is
done purposefully, because the feature has security implications. Sometimes the
feature will be implemented at a later time, these CVEs might become
relevant at that time.

| CVE            | Reason                                                                                                      |
| -------------- | ----------------------------------------------------------------------------------------------------------- |
| CVE-2002-0043  | mail functionality is not implemented, https://www.sudo.ws/security/advisories/postfix/                     |
| CVE-2005-2959  | env_reset is always enabled / blacklist is not supported, https://www.sudo.ws/security/advisories/bash_env/ |
| CVE-2005-4158  | env_reset is always enabled / blacklist is not supported, https://www.sudo.ws/security/advisories/perl_env/ |
| CVE-2006-0151  | env_reset is always enabled / blacklist is not supported                                                    |
| CVE-2007-3149  | Kerberos functionality is not implemented, https://www.sudo.ws/security/advisories/kerberos5/               |
| CVE-2009-0034  | The group matching logic does not have this bug, https://www.sudo.ws/security/advisories/group_vector/      |
| CVE-2010-0427  | runas_default is not implemented                                                                            |
| CVE-2012-2337  | No host ip-based rule matching is currently implemented, https://www.sudo.ws/security/advisories/netmask/   |
| CVE-2012-3440  | Related to Red Hat specific script and not sudo directly                                                    |
| CVE-2014-0106  | Disabling env_reset is not supported, https://www.sudo.ws/security/advisories/env_add/                      |
| CVE-2015-8239  | The sha2 digest feature is not implemented                                                                  |
| CVE-2019-14287 | This bug is not present, https://www.sudo.ws/security/advisories/minus_1_uid/                               |
| CVE-2021-23240 | sudo-rs does not have SELinux support, https://www.sudo.ws/security/advisories/sudoedit_selinux/            |
| CVE-2022-43995 | crypt/password backend is not implemented, only PAM                                                         |
| CVE-2023-27320 | The chroot functionality is not implemented, https://www.sudo.ws/security/advisories/double_free/           |
| CVE-2023-28487 | Sudoreplay is not implemented                                                                               |
| CVE-2025-32462 | `sudo -h` is not implemented, https://www.sudo.ws/security/advisories/host_any/                             |
| CVE-2025-32463 | The chroot functionality is not implemented, https://www.sudo.ws/security/advisories/chroot_bug/            |

## Disputed CVEs

While these CVEs are related to sudo, they are disputed as security issues.
Either the behavior described in the CVE is intended behavior, or the issue
cannot be replicated.

| CVE            | Notes |
| -------------- | ----- |
| CVE-2005-1831  |       |
| CVE-2019-18684 |       |
| CVE-2019-19234 |       |
| CVE-2019-19232 |       |
