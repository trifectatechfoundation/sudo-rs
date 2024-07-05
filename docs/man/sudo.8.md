<!-- ---
title: SUDO(8) sudo-rs 0.2.3 | sudo-rs
--- -->

# NAME

`sudo` - execute a command as another user

# SYNOPSIS

`sudo` [`-u` *user*] [`-g` *group*] [`-D` *directory*] [`-knS`] [`-i` | `-s`] [<*command*>] \
`sudo` `-h` | `-K` | `-k` | `-V`

# DESCRIPTION

`sudo` allows a user that is permitted to do so to execute a *command* as
another user (for example *root*). Permissions are specified by a security
policy specified in `/etc/sudoers` (see sudoers(5)).

Sudo-rs is a safety oriented and memory safe re-implementation of the original
sudo implementation by Todd Miller.

When a command is run, a session record is stored for that specific session
allowing users to run additional commands without having to re-authenticate. The
timeout for session records can be specified in the policy.

Some care is taken to pass signals received by sudo-rs to the child process,
even if that process runs in its own pseudo terminal.

# OPTIONS

`-D` *directory*, `--chdir`=*directory*
:   Run the *command* in the specified *directory* instead of the current
    working directory. The security policy may return an error if the user does
    not have the permission to specify the working directory.

`-g` *group*, `--group`=*group*
:   Use this *group* as the primary group instead of using the primary group
    specified in the password database for the target user.

`-h`, `--help`
:   Show a help message.

`-i`, `--login`
:   Run the shell specified by the target user's password database entry as a
    login shell. This means that login-specific resource files such as
    *.profile*, *.bash_profile* or *.login* will be read by the shell. If a
    *command* is specified, it is passed to the shell using the `-c` option.

`-K`, `--remove-timestamp`
:   Removes every cached session record for the user, regardless of where the
    command is executed. The next time sudo-rs is run, authentication will take
    place if the policy requires it. No password is required to run this
    command.

`-k`, `--reset-timestamp`
:   When used without a command, invalidates the user's session record for
    the current session. The next time sudo-rs is run, authentication will take
    place if the policy requires it.

    When used in conjunction with a *command* or an option that may require a
    password, this option will cause sudo-rs to ignore the user's session
    record. As a result, authentication will take place if the policy requires
    it. When used in conjunction with a *command* no invalidation of existing
    session records will take place.

`-n`, `--non-interactive`
:   Avoid prompting the user for input of any kind. If any input is required for
    the *command* to run, sudo-rs will display an error message and exit.

`-S`, `--stdin`
:   Read from standard input instead of using the terminal device.

`-s`, `--shell`
:   Run the shell specified by the `SHELL` environment variable. If no shell
    was specified, the shell from the user's password database entry will be
    used instead. If a *command* is specified, it is passed to the shell using
    the `-c` option.

`-u` *user*, `--user`=*user*
:   Run the *command* as another user than the default (**root**).

`-V`, `--version`
:   Display the current version of sudo-rs.

`-v`, `--validate`
:   Update the session record for the current session, authenticating the user
    if necessary.

`--`
:   Indicates the end of the sudo-rs options and start of the *command*.

# SEE ALSO

[su(1)](su.1.md), sudoers(5), [visudo(8)](visudo.8.md)
