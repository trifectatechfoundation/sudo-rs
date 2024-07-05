<!-- ---
title: VISUDO(8) sudo-rs 0.2.3 | sudo-rs
--- -->

# NAME

`visudo` - safely edit the sudoers file

# SYNOPSIS

`visudo` [`-chqsV`] [[`-f`] *sudoers*]

# DESCRIPTION

`visudo` edits the *sudoers* file in a safe manner, similar to vipw(8).

# OPTIONS

`-c`, `--check`
:   Only check if there are errors in the existing sudoers file.

`-f` *sudoers*, `--file`=*sudoers*
:   Instead of editing the default `/etc/sudoers`, edit the file specified as
    *sudoers* instead.

`-h`, `--help`
:   Show a help message.

`-I`, `--no-includes`
:   Do not edit included files.

`-q`, `--quiet`
:   Less verbose syntax error messages.

`-s`, `--strict`
:   Strict syntax checking.

`-V`, `--version`
:   Display version information and exit.

# SEE ALSO

[sudo(8)](sudo.8.md), sudoers(5)
