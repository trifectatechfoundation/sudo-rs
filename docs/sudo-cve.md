# Past sudo CVEs

This listing contains security issues originally found in sudo but which could
also be relevant for sudo-rs.

## Possibly relevant CVEs / advisories

These CVEs/advisories are possibly relevant to sudo-rs:

| CVE              | Tests | Advisory & notes                                             |
| ---------------- | ----- | ------------------------------------------------------------ |
| CVE-1999-0958    |       | Relative path (.. attack)                                    |
| CVE-1999-1496    |       | Information leakage on which commands exist                  |
| -                |       | https://www.sudo.ws/security/advisories/heap_corruption/     |
| CVE-2004-1051    |       | https://www.sudo.ws/security/advisories/bash_functions/      |
| CVE-2005-1993    |       | https://www.sudo.ws/security/advisories/path_race/           |
| CVE-2005-4890    |       | use_pty is enabled by default in sudo-rs                     |
| CVE-2009-0034    |       | https://www.sudo.ws/security/advisories/group_vector/        |
| -                |       | https://www.sudo.ws/security/advisories/cmnd_alias_negation/ |
| CVE-2010-1646    |       | https://www.sudo.ws/security/advisories/secure_path/         |
| CVE-2010-2956    |       | https://www.sudo.ws/security/advisories/runas_group/         |
| CVE-2011-0010    |       | https://www.sudo.ws/security/advisories/runas_group_pw/      |
| CVE-2012-0809    |       | https://www.sudo.ws/security/advisories/sudo_debug/          |
| CVE-2013-1775    |       | https://www.sudo.ws/security/advisories/epoch_ticket/        |
| CVE-2013-1776    |       | https://www.sudo.ws/security/advisories/tty_tickets/         |
| CVE-2013-2776    |       | https://www.sudo.ws/security/advisories/tty_tickets/         |
| CVE-2013-2777    |       | https://www.sudo.ws/security/advisories/tty_tickets/         |
| CVE-2014-9680    |       | https://www.sudo.ws/security/advisories/tz/                  |
| CVE-2017-1000367 |       | https://www.sudo.ws/security/advisories/linux_tty/           |
| CVE-2017-1000368 |       | https://www.sudo.ws/security/advisories/linux_tty/           |
| CVE-2019-14287   |       | https://www.sudo.ws/security/advisories/minus_1_uid/         |
| CVE-2023-28486   |       | log message control character escapes                        |

## Non-applicable CVEs

These CVEs are almost entirely not applicable in the current sudo-rs codebase,
mainly because the feature they relate to is not implemented. Sometimes this is
done purposefully, because the feature has security implications. Sometimes the
feature will be implemented at a later time, these CVEs might become
relevant at that time.

| CVE            | Reason                                                                                                      |
| -------------- | ----------------------------------------------------------------------------------------------------------- |
| CVE-2002-0043  | mail functionality is not implemented, https://www.sudo.ws/security/advisories/postfix/                     |
| CVE-2002-0184  | setting a custom prompt via `-p` is not implemented, https://www.sudo.ws/security/advisories/prompt/        |
| CVE-2004-1689  | `sudoedit`/`sudo -e` is not implemented, https://www.sudo.ws/security/advisories/sudoedit/                  |
| CVE-2005-1119  | `visudo` functionality is currently not implemented                                                         |
| CVE-2005-2959  | env_reset is always enabled / blacklist is not supported, https://www.sudo.ws/security/advisories/bash_env/ |
| CVE-2005-4158  | env_reset is always enabled / blacklist is not supported, https://www.sudo.ws/security/advisories/perl_env/ |
| CVE-2006-0151  | env_reset is always enabled / blacklist is not supported                                                    |
| CVE-2007-3149  | Kerberos functionality is not implemented, https://www.sudo.ws/security/advisories/kerberos5/               |
| CVE-2010-0426  | `sudoedit`/`sudo -e` is not implemented, https://www.sudo.ws/security/advisories/sudoedit_escalate/         |
| CVE-2010-0427  | runas_default is not implemented                                                                            |
| CVE-2010-1163  | `sudoedit`/`sudo -e` is not implemented, https://www.sudo.ws/security/advisories/sudoedit_escalate2/        |
| CVE-2012-2337  | No host-based rule matching is currently implemented, https://www.sudo.ws/security/advisories/netmask/      |
| CVE-2012-3440  | Related to Red Hat specific script and not sudo directly                                                    |
| CVE-2014-0106  | Disabling env_reset is not supported, https://www.sudo.ws/security/advisories/env_add/                      |
| CVE-2015-5602  | `sudoedit`/`sudo -e` is not implemented                                                                     |
| CVE-2015-8239  | The sha2 digest feature is not implemented                                                                  |
| CVE-2016-7032  | The noexec functionality is not implemented, https://www.sudo.ws/security/advisories/noexec_bypass/         |
| CVE-2016-7076  | The noexec functionality is not implemented, https://www.sudo.ws/security/advisories/noexec_wordexp/        |
| CVE-2019-18634 | The pwfeedback functionality is not implemented, https://www.sudo.ws/security/advisories/pwfeedback/        |
| CVE-2021-3156  | `sudoedit`/`sudo -e` is not implemented, https://www.sudo.ws/security/advisories/unescape_overflow/         |
| CVE-2021-23239 | `sudoedit`/`sudo -e` is not implemented                                                                     |
| CVE-2021-23240 | `sudoedit`/`sudo -e` is not implemented, https://www.sudo.ws/security/advisories/sudoedit_selinux/          |
| CVE-2022-43995 | crypt/password backend is not implemented, only PAM                                                         |
| CVE-2023-22809 | `sudoedit`/`sudo -e` is not implemented, https://www.sudo.ws/security/advisories/sudoedit_any/              |
| CVE-2023-27320 | The chroot functionality is not implemented, https://www.sudo.ws/security/advisories/double_free/           |
| CVE-2023-28487 | Sudoreplay is not implemented                                                                               |

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
