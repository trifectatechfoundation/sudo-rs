---
title: SUDO(8) sudo-rs 0.2.11 | sudo-rs
---

# NAME

`sudo`, `sudoedit` - execute a command as another user

# SYNOPSIS

`sudo` `-h` | `-K` | `-k` | `-V`\
`sudo` \[`-u` *user*\] \[`-g` *group*\] \[`-D` *directory*\] \[`-BknS`\] \[`-i` | `-s`\] \[`VAR=value`\] \[<*command*>\]\
`sudo` `-v` \[`-BknS`\] \[`-u` *user*\]  \[`-g` *group*\]\
`sudo` `-l` \[`-BknS`\] \[`-U` *user*\] \[`-u` *user*\]  \[`-g` *group*\] \[command \[arg ...\]\]\
`sudo` `-e` \[`-BknS`\] \[`-u` *user*\] \[`-g` *group*\] file ...\
`sudoedit` \[`-BknS`\] \[`-u` *user*\] \[`-g` *group*\] file ...


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

On systems where sudo is the primary method of gaining superuser privileges, it is
imperative to avoid syntax errors in the `/etc/sudoers` file. Changes to this file
should be made using the visudo(8) utility which will ensure that no syntax errors
are introduced.

# OPTIONS

`-B`, `--bell`
: Ring the bell as part of the password prompt when a terminal is present.

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

`p`, `--prompt`=*prompt*
:   Use a custom authentication prompt with optional escape sequences. The
    following percent (‘%’) escape sequences are supported:

         %H  expanded to the local host name

         %h  expanded to the local host name without the domain name

         %p  expanded to the name of the user whose password is being requested
             (this respects the rootpw, targetpw flags)

         %U  expanded to the login name of the user the command will be run as
             (defaults to root unless the -u option is also specified)

         %u  expanded to the invoking user's login name

         %%  two consecutive ‘%’ characters are collapsed into a single ‘%’ character

    The custom prompt will override the default prompt or the one specified by the SUDO_PROMPT environment variable.
    No *prompt* will suppress the prompt provided by PAM, unless the requested *prompt* is empty (`""`)

`-S`, `--stdin`
:   Read from standard input instead of using the terminal device.

`-s`, `--shell`
:   Run the shell specified by the `SHELL` environment variable. If no shell
    was specified, the shell from the user's password database entry will be
    used instead. If a *command* is specified, it is passed to the shell using the `-c` option.

`-e`, `sudoedit`

    Edit one or more files instead of running a command.  In lieu of a path name, the string "sudoedit" is used when consulting the security policy.  If the user is authorized by the policy, the following steps are taken:

    1. Temporary copies are made of the files to be edited with the owner set to the invoking user.

    2. The editor specified by the policy is run to edit the temporary files.  The sudoers policy uses the SUDO_EDITOR, VISUAL and EDITOR environment variables (in that order).  If none of SUDO_EDITOR, VISUAL or EDITOR are set, the first program listed in the editor sudoers(5) option is used.

    3. If they have been modified, the content of the temporary files is copied back to the originals and the temporary versions are removed.

    To help prevent the editing of unauthorized files, the following restrictions are enforced (unless the user is root):

    * Symbolic links may not be edited.

    * If any component of the path leading to the file is writable by the invoking user, the file may not be edited.

    * Users are never allowed to edit device special files.

    If the specified file does not exist, it will be created. Unlike most commands run by sudo, the editor is run with the invoking user's environment unmodified. If the temporary file becomes empty after editing, the user will be prompted before it is installed.

`-u` *user*, `--user`=*user*
:   Run the *command* as another user than the default (**root**).

`-V`, `--version`
:   Display the current version of sudo-rs.

`-v`, `--validate`
:   Update the session record for the current session, authenticating the user
    if necessary.

`-l`, `--list`
:   List user's privileges or check a specific command; use twice for longer format

`-U`, `--other-user`=*user*
:   Used in list mode, display privileges for another user


`--`
:   Indicates the end of the sudo-rs options and start of the *command*.

Environment variables to be set for the command may be passed on the command line in the form of VAR=value. Variables passed on the command line are subject to restrictions imposed by the security policy.
Variables passed on the command line are subject to the same restrictions as normal environment variables with one important exception: If the command to be run has the SETENV tag set or the command matched is ALL,
the user may set variables that would otherwise be forbidden. See [sudoers(5)](sudoers.5.md) for more information.

# SEE ALSO

[su(1)](su.1.md), [sudoers(5)](sudoers.5.md), [visudo(8)](visudo.8.md)
