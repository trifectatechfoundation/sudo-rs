---
title: SUDOERS(5) sudo-rs 0.2.12 | sudo-rs
---

# NAME

`sudoers` - sudo-compatible security configuration

# DESCRIPTION

The `sudo-rs` policy determines a user's sudo privileges. The policy is driven by the */etc/sudoers file*.  The policy format is described in detail in the **SUDOERS FILE FORMAT** section.

The format used by sudo-rs is a subset of the one used by the sudo-project as maintained by Todd Miller, but syntax-compatible.

## User Authentication

The sudoers security policy requires that most users authenticate themselves before they can use sudo.  A password is not required if the invoking user is root, if the target user is the same as the invoking user, or if the policy has disabled authentication for the user or command.  Unlike `su`, when `sudo-rs` requires authentication, it validates the invoking user's credentials, not the target user's (or root's) credentials.  This can be changed via the *rootpw* flag, described later.

`sudo-rs` uses per-user timestamp files for credential caching.  Once a user has been authenticated, a record is written containing the user-ID that was used to authenticate, the terminal session ID, the start time of the session leader (or parent process) and a timestamp (using a monotonic clock if one is available).  The user may then use sudo without a password for a short period of time (15 minutes unless overridden by the timestamp_timeout option).  `sudo-rs` uses a separate record for each terminal, which means that a user's login sessions are authenticated separately.

## Logging

By default, `sudo-rs` logs both successful and unsuccessful attempts (as well as errors).  Messages are logged to syslog(3).

## Command environment

Since environment variables can influence program behavior, `sudo-rs` restricts which variables from the user's environment are inherited by the command to be run.

In `sudo-rs`, the *env_reset* flag cannot be disabled. This causes commands to be executed with a new, minimal environment.
The `HOME`, `SHELL`, `LOGNAME` and `USER` environment variables are initialized based on the target user and the `SUDO_*` variables are set based on the invoking user.  Additional variables, such as `DISPLAY`, `PATH` and `TERM`, are preserved from the invoking user's environment if permitted by the *env_check* or *env_keep* options. A few environment variables are treated specially. If the `PATH` and `TERM` variables are not preserved from the user's environment, they will be set to default values.  The `LOGNAME` and `USER` are handled as a single entity.  If one of them is preserved (or removed) from the user's environment, the other will be as well.
If `LOGNAME` and `USER` are to be preserved but only one of them is present in the user's environment, the other will be set to the same value.  This avoids an inconsistent environment where one of the variables describing the user name is set to the invoking user and one is set to the target user.
Environment variables with a value beginning with `()` are removed, as they may be interpreted as functions by the bash shell.

Environment variables specified by *env_check* or *env_keep* may include one or more ‘\*’ characters which will match zero or more characters.
No other wildcard characters are supported. Other sudoers options may influence the command environment, such as *secure_path*.

Variables in the PAM environment may be merged in to the environment.  If a variable in the PAM environment is already present in the user's environment, the value will only be overridden if the variable was not preserved by `sudo-rs`. Variables preserved from the invoking user's environment by the *env_keep* list take precedence over those in the PAM environment.

Note that the dynamic linker on most operating systems will remove variables that can control dynamic linking from the environment of set-user-ID executables, including sudo.  Depending on the operating system this may include `_RLD*`, `DYLD_*`, `LD_*`, `LDR_*`, `LIBPATH`, `SHLIB_PATH`, and others.  These type of variables are removed from the environment before sudo even begins execution and, as such, it is not possible for sudo to preserve them.

## Resource limits

sudo uses the operating system's native method of setting resource limits for the target user. On Linux systems, resource limits are usually set by the *pam_limits.so* PAM module. On some BSD systems, the */etc/login.conf* file specifies resource limits for the user. If there is no system mechanism to set per-user resource limits, the command will run with the same limits as the invoking user.

# SUDOERS FILE FORMAT

The sudoers file is composed of two types of entries: aliases (basically variables) and user specifications (which specify who may run what).

When multiple entries match for a user, they are applied in order.  Where there are multiple matches, the last match is used (which is not necessarily the most specific match).

The sudoers file grammar will be described below in Extended Backus-Naur Form (EBNF) borrowed from Todd Miller's sudoers documentation.

## Quick guide to EBNF

EBNF is a concise and exact way of describing the grammar of a language.  Each EBNF definition is made up of production rules.  E.g.,

     symbol ::= definition | alternate1 | alternate2 ...

Each production rule references others and thus makes up a grammar for the language.  EBNF also contains the following operators, which many readers will recognize from regular expressions.  Do not, however, confuse them with “wildcard” characters, which have different meanings.

     ?     Means that the preceding symbol (or group of symbols) is optional.  That is, it may appear once or not at all.

     *     Means that the preceding symbol (or group of symbols) may appear zero or more times.

     +     Means that the preceding symbol (or group of symbols) may appear one or more times.

Parentheses may be used to group symbols together.  For clarity, we will use single quotes ('') to designate what is a verbatim character string (as opposed to a symbol name).

## Aliases

There are four kinds of aliases: User_Alias, Runas_Alias, Host_Alias and Cmnd_Alias.

     Alias ::= 'User_Alias'  User_Alias_Spec (':' User_Alias_Spec)* |
               'Runas_Alias' Runas_Alias_Spec (':' Runas_Alias_Spec)* |
               'Host_Alias'  Host_Alias_Spec (':' Host_Alias_Spec)* |
               'Cmnd_Alias'  Cmnd_Alias_Spec (':' Cmnd_Alias_Spec)* |
               'Cmd_Alias'   Cmnd_Alias_Spec (':' Cmnd_Alias_Spec)*

     User_Alias ::= NAME

     User_Alias_Spec ::= User_Alias '=' User_List

     Runas_Alias ::= NAME

     Runas_Alias_Spec ::= Runas_Alias '=' Runas_List

     Host_Alias ::= NAME

     Host_Alias_Spec ::= Host_Alias '=' Host_List

     Cmnd_Alias ::= NAME

     Cmnd_Alias_Spec ::= Cmnd_Alias '=' Cmnd_List

     NAME ::= [A-Z]([A-Z][0-9]_)*

Each alias definition is of the form

     Alias_Type NAME = item1, item2, ...

where *Alias_Type* is one of User_Alias, Runas_Alias, Host_Alias, or Cmnd_Alias. A NAME is a string of uppercase letters, numbers, and underscore characters (‘_’).  A NAME must start with an uppercase letter.  It is possible to put several alias definitions of the same type on a single line, joined by a colon (‘:’).  E.g.,

     Alias_Type NAME = item1, item2, item3 : NAME = item4, item5

The definitions of what constitutes a valid alias member follow.

     User_List ::= User |
                   User ',' User_List

     User ::= '!'* user name |
              '!'* #user-ID |
              '!'* %group |
              '!'* %#group-ID |
              '!'* User_Alias

A User_List is made up of one or more user names, user-IDs (prefixed with ‘#’), system group names and IDs (prefixed with ‘%’ and ‘%#’ respectively)
and User_Aliases. Each list item may be prefixed with zero or more ‘!’ operators.  An odd number of ‘!’ operators negate the value of the item; an even number just cancel each other out.

     Runas_List ::= Runas_Member |
                    Runas_Member ',' Runas_List

     Runas_Member ::= '!'* user name |
                      '!'* #user-ID |
                      '!'* %group |
                      '!'* %#group-ID |
                      '!'* Runas_Alias

A Runas_List is similar to a User_List except that instead of User_Aliases it can contain Runas_Aliases.  Note that user names and groups are matched as strings.  In other words, two users (groups) with the same user (group) ID are considered to be distinct.  If you wish to match all user names with the same user-ID (e.g., root and toor), you can use a user-ID instead of a name (`#0` in the example given).

     Host_List ::= Host |
                   Host ',' Host_List

     Host ::= '!'* host name |
              '!'* Host_Alias

A Host_List is made up of one or more host names.  Again, the value of an item may be negated with the ‘!’ operator.

     Cmnd_List ::= Cmnd |
                   Cmnd ',' Cmnd_List

     command name ::= file name |
                      file name args ['*'] |
                      file name '""'

     Cmnd ::= '!'* command name |
              '!'* directory |
              '!'* Cmnd_Alias
              '!'* "list"
              '!'* "sudoedit" [file name]

A Cmnd_List is a list of one or more command names, directories, and other aliases.  A command name is a fully qualified file name which may include shell-style wildcards (see the Wildcards section below).  A simple file name allows the user to run the command with any arguments they wish.  However, you may also specify command line arguments that have to be used, in which case the command line has to match exactly. You can use the special argument "" to indicate that the command may only be run *without* command line arguments, or the argument ‘*’ to match any trailing arguments. You cannot use wildcards inside the argument list.  A directory is a fully qualified path name ending in a ‘/’.  When you specify a directory in a Cmnd_List, the user will be able to run any file within that directory (but not in any sub-directories therein).

If a Cmnd has associated command line arguments, then the arguments in the Cmnd must match exactly those given by the user on the command line.
Note that the following characters must be escaped with a ‘\\’ if they are used in command arguments: ‘,’, ‘:’, ‘=’, ‘\\’.

There are two commands built into sudo itself: “list” and “sudoedit”.  Unlike other commands, these two must be specified in the sudoers file without a leading path.

The “list” built-in can be used to permit a user to list another user's privileges with sudo's -U option.  For example, “sudo -l -U otheruser”.  A user
with the “list” privilege is able to list another user's privileges even if they don't have permission to run commands as that user.  By default, only
root or a user with the ability to run any command as either root or the specified user on the current host may use the -U option.  No command line arguments may
be specified with the “list” built-in.

The “sudoedit” built-in is used to permit a user to run sudo with the -e option (or as sudoedit). It may take command line arguments just as a normal command does. Unlike other commands, “sudoedit” is built into sudo itself and must be specified in the sudoers file without a leading path.
If a leading path is present, for example /usr/bin/sudoedit, this will not give the user permissions to use sudoedit. If no arguments are provided, “sudoedit” will give the user the permission to edit any files; if an argument is present it must be an absolute path name that does not contain symbolic links, or the command will not be matched.

## Defaults

Certain configuration options may be changed from their default values at run-time via one or more Default_Entry lines.  These may affect all users on any host, all users on a specific host, a specific user, a specific command, or commands being run as a specific user.  Note that per-command entries may not include command line arguments.  If you need to specify arguments, define a Cmnd_Alias and reference that instead.

     Default_Type ::= 'Defaults' |
                      'Defaults' '@' Host_List |
                      'Defaults' ':' User_List |
                      'Defaults' '!' Cmnd_List |
                      'Defaults' '>' Runas_List

     Default_Entry ::= Default_Type Parameter_List

     Parameter_List ::= Parameter |
                        Parameter ',' Parameter_List

     Parameter ::= Parameter '=' Value |
                   Parameter '+=' Value |
                   Parameter '-=' Value |
                   '!'* Parameter

Parameters may be flags, integer values, strings, or lists.  Flags are implicitly boolean and can be turned off via the ‘!’ operator.  Some integer, string and list parameters may also be used in a boolean context to disable them.  Values may be enclosed in double quotes ("") when they contain multiple words.  Special characters may be escaped with a backslash (‘\\’).

To include a literal backslash character in a command line argument you must escape the backslash twice.  For example, to match ‘\\n’ as part of a command line argument, you must use ‘\\\\\\\\n’ in the sudoers file.  This is due to there being two levels of escaping, one in the sudoers parser itself and another when command line arguments are matched by the fnmatch(3) function.

Lists have two additional assignment operators, *+=* and *-=*.  These operators are used to add to and delete from a list respectively.  It is not an error to use the -= operator to remove an element that does not exist in a list.

Defaults entries are parsed in the following order: generic, host, user, and runas Defaults are processed in the order they appear, with per-command defaults being processed in a second pass after that.

See **SUDOERS OPTIONS** for a list of supported Defaults parameters.

## User specification

     User_Spec ::= User_List Host_List '=' Cmnd_Spec_List \
                   (':' Host_List '=' Cmnd_Spec_List)*

     Cmnd_Spec_List ::= Cmnd_Spec |
                        Cmnd_Spec ',' Cmnd_Spec_List

     Cmnd_Spec ::= Runas_Spec? Chdir_Spec? Tag_Spec* Cmnd

     Runas_Spec ::= '(' Runas_List? (':' Runas_List)? ')'

     Chdir_Spec ::= 'CWD=directory'

     Tag_Spec ::= ('PASSWD:' | 'NOPASSWD:' |
                   'SETENV:' | 'NOSETENV:'
                   'EXEC:'   | 'NOEXEC')

     AppArmor_Spec ::= 'APPARMOR_PROFILE=profile'

A user specification determines which commands a user may run (and as what user) on specified hosts.  By default, commands are run as root, but this can be changed on a per-command basis.

The basic structure of a user specification is “who where = (as_whom) what”.  Let's break that down into its constituent parts:

## Runas_Spec

A Runas_Spec determines the user and/or the group that a command may be run as.  A fully-specified Runas_Spec consists of two Runas_Lists (as defined above) separated by a colon (‘:’) and enclosed in a set of parentheses.  The first Runas_List indicates which users the command may be run as via the -u option.  The second defines a list of groups that may be specified via the -g option (in addition to any of the target user's groups).  If both Runas_Lists are specified, the command may be run with any combination of users and groups listed in their respective Runas_Lists. If only the first is specified, the command may be run as any user in the list and, optionally, with any group the target user belongs to.  If the first Runas_List is empty but the second is specified, the command may be run as the invoking user with the group set to any listed in the Runas_List.  If both Runas_Lists are empty, the command may only be run as the invoking user and the group, if specified, must be one that the invoking user is a member of.  If no Runas_Spec is specified, the command may only be run as root and the group, if specified, must be one that root is a member of.

A Runas_Spec sets the default for the commands that follow it.  What this means is that for the entry:

     dgb     boulder = (operator) /bin/ls, /bin/kill, /usr/bin/lprm

The user dgb may run /bin/ls, /bin/kill, and /usr/bin/lprm on the host boulder—but only as operator.  E.g.,

     $ sudo -u operator /bin/ls

It is also possible to override a Runas_Spec later on in an entry.  If we modify the entry like so:

     dgb     boulder = (operator) /bin/ls, (root) /bin/kill, /usr/bin/lprm

Then user dgb is now allowed to run /bin/ls as operator, but /bin/kill and /usr/bin/lprm as root.

We can extend this to allow dgb to run /bin/ls with either the user or group set to operator:

     dgb     boulder = (operator : operator) /bin/ls, (root) /bin/kill,\
             /usr/bin/lprm

Note that while the group portion of the Runas_Spec permits the user to run as command with that group, it does not force the user to do so.  If no group is specified on the command line, the command will run with the group listed in the target user's password database entry.  The following would all be permitted by the sudoers entry above:

     $ sudo -u operator /bin/ls
     $ sudo -u operator -g operator /bin/ls
     $ sudo -g operator /bin/ls

In the following example, user tcm may run commands that access a modem device file with the dialer group.

     tcm     boulder = (:dialer) /usr/bin/tip, /usr/bin/cu,\
             /usr/local/bin/minicom

Note that in this example only the group will be set, the command still runs as user tcm.  E.g.

     $ sudo -g dialer /usr/bin/cu

Multiple users and groups may be present in a Runas_Spec, in which case the user may select any combination of users and groups via the -u and -g options.  In this example:

     alan    ALL = (root, bin : operator, system) ALL

user alan may run any command as either user root or bin, optionally setting the group to operator or system.

## Chdir_Spec

The working directory that the command will be run in can be specified using the CWD setting.  The directory must be a fully-qualified path name beginning with a ‘/’ or ‘~’ character, or the special value “\*”.  A value of “\*” indicates that the user may specify the working directory by running sudo with the -D option.  By default, commands are run from the invoking user's current working directory, unless the -i option is given.  Path names of the form ~user/path/name are interpreted as being relative to the named user's home directory.  If the user name is omitted, the path will be relative to the runas user's home directory.

## Tag_Spec

A command may have zero or more tags associated with it.  The following tag values are supported: PASSWD, NOPASSWD, SETENV, and NOSETENV.
Once a tag is set on a Cmnd, subsequent Cmnds in the Cmnd_Spec_List, inherit the tag unless it is overridden by the opposite tag (in other words, PASSWD overrides NOPASSWD and NOSETENV overrides SETENV).

### EXEC and NOEXEC

On Linux systems, the NOEXEC tag can be used to prevent an executable from running further commands itself.

In the following example, user aaron may run /usr/bin/more and /usr/bin/vi but shell escapes will be disabled.

        aaron   shanty = NOEXEC: /usr/bin/more, /usr/bin/vi

See the _Preventing shell escapes_ section below for more details on how NOEXEC works and whether or not it suits your purpose.

### PASSWD and NOPASSWD

By default, sudo requires that a user authenticate before running a command.  This behavior can be modified via the NOPASSWD tag.  Like a Runas_Spec, the NOPASSWD tag sets a default for the commands that follow it in the Cmnd_Spec_List.  Conversely, the PASSWD tag can be used to reverse things.  For example:

       queen     rushmore = NOPASSWD: /bin/kill, /bin/ls, /usr/bin/lprm

would allow the user queen to run /bin/kill, /bin/ls, and /usr/bin/lprm as root on the machine “rushmore” without authenticating himself.  If we only want queen to be able to run /bin/kill without a password the entry would be:

       queen     rushmore = NOPASSWD: /bin/kill, PASSWD: /bin/ls, /usr/bin/lprm

By default, if the NOPASSWD tag is applied to any of a user's entries for the current host, the user will be able to run “sudo -l” without a password.  Additionally, a user may only run “sudo -v” without a password if all of the user's entries for the current host have the NOPASSWD tag.

### SETENV and NOSETENV

These tags override the value of the setenv flag on a per-command basis.  Note that if SETENV has been set for a command, the user may disable the env_reset flag from the command line via the -E option.  Additionally, environment variables set on the command line are not subject to the restrictions imposed by env_check, env_delete, or env_keep.  As such, only trusted users should be allowed to set variables in this manner.  If the command matched is ALL, the SETENV tag is implied for that command; this default may be overridden by use of the NOSETENV tag.

## AppArmor_Spec
When sudo-rs is built with support for AppArmor, sudoers file entries may specify an AppArmor profile that should be used to confine a command.

If an AppArmor profile is specified with the command, it will override any default values specified in sudoers. Appropriate profile transition rules must be defined to support the profile change specified for a user.

AppArmor profiles can be specified in any way that complies with the rules of `aa_change_profile(2)`.

## Wildcards

sudo allows shell-style wildcards (aka meta or glob characters) to be used in host names, path names, and command line arguments in the sudoers file.  Wildcard matching is done via the glob(3) and fnmatch(3) functions as specified by IEEE Std 1003.1 (“POSIX.1”).

     *         Matches any set of zero or more characters (including white space).

     ?         Matches any single character (including white space).

     [...]     Matches any character in the specified range.

     [!...]    Matches any character not in the specified range.

     \x        For any character ‘x’, evaluates to ‘x’.  This is used to escape special characters such as: ‘*’, ‘?’, ‘[’, and ‘]’.

Note that these are not regular expressions.  Unlike a regular expression there is no way to match one or more characters within a range.

Wildcards in command line arguments are not supported---using these in original versions of sudo was usually a sign of mis-configuration and consequently sudo-rs simply forbids using them. The only supported use is ‘*’ as the final argument to indicate "zero or more subsequent arguments" as noted above.

## Including other files from within sudoers

It is possible to include other sudoers files from within the sudoers file currently being parsed using the *@include* and *@includedir* directives.  For compatibility with Todd Miller's sudo versions prior to 1.9.1, *#include* and *#includedir* are also accepted.

An include file can be used, for example, to keep a site-wide sudoers file in addition to a local, per-machine file.  For the sake of this example the site-wide sudoers file will be /etc/sudoers and the per-machine one will be /etc/sudoers.local.  To include /etc/sudoers.local from within /etc/sudoers one would use the following line in /etc/sudoers:

         @include /etc/sudoers.local

When sudo reaches this line it will suspend processing of the current file (/etc/sudoers) and switch to /etc/sudoers.local.  Upon reaching the end of /etc/sudoers.local, the rest of /etc/sudoers will be processed.  Files that are included may themselves include other files.  A hard limit of 128 nested include files is enforced to prevent include file loops.

The path to the include file may contain white space if it is escaped with a backslash (‘\\’).  Alternately, the entire path may be enclosed in double quotes (""), in which case no escaping is necessary.  To include a literal backslash in the path, ‘\\\\’ should be used. If the path to the include file is not fully-qualified (does not begin with a ‘/’), it must be located in the same directory as the sudoers file it was included from.  For example, if /etc/sudoers contains the line:

         @include sudoers.local

The @includedir directive can be used to create a sudoers.d directory that the system package manager can drop sudoers file rules into as part of package installation.  For example, given:

         @includedir /etc/sudoers.d

sudo will suspend processing of the current file and read each file in /etc/sudoers.d, skipping file names that end in ‘~’ or contain a ‘.’ character to avoid causing problems with package manager or editor temporary/backup files.  Files are parsed in sorted lexical order.  That is, /etc/sudoers.d/01_first will be parsed before /etc/sudoers.d/10_second.  Be aware that because the sorting is lexical, not numeric, /etc/sudoers.d/1_whoops would be loaded after /etc/sudoers.d/10_second.  Using a consistent number of leading zeroes in the file names can be used to avoid such problems.  After parsing the files in the directory, control returns to the file that contained the @includedir directive.

Note that unlike files included via @include, visudo will not edit the files in a @includedir directory unless one of them contains a syntax error.  It is still possible to run visudo with the -f flag to edit the files directly, but this will not catch the redefinition of an alias that is also present in a different file.

## Other special characters and reserved words

The pound sign (‘#’) is used to indicate a comment (unless it is part of a #include directive or unless it occurs in the context of a user name and is followed by one or more digits, in which case it is treated as a user-ID).  Both the comment character and any text after it, up to the end of the line, are ignored.

The reserved word *ALL* is a built-in alias that always causes a match to succeed.  It can be used wherever one might otherwise use a Cmnd_Alias, User_Alias, Runas_Alias, or Host_Alias.  Attempting to define an alias named ALL will result in a syntax error.  Please note that using ALL can be dangerous since in a command context, it allows the user to run any command on the system.

An exclamation point (‘!’) can be used as a logical not operator in a list or alias as well as in front of a Cmnd.  This allows one to exclude certain values.  For the ‘!’ operator to be effective, there must be something for it to exclude.  For example, to match all users except for root one would use:

         ALL,!root

If the ALL, is omitted, as in:

         !root

it would explicitly deny root but not match any other users.  This is different from a true “negation” operator.

Note, however, that using a ‘!’ in conjunction with the built-in ALL alias to allow a user to run “all but a few” commands rarely works as intended (see SECURITY NOTES below).

White space between elements in a list as well as special syntactic characters in a User Specification (‘=’, ‘:’, ‘(’, ‘)’) is optional.

The following characters must be escaped with a backslash (‘\\’) when used as part of a word (e.g., a user name or host name): ‘!’, ‘=’, ‘:’, ‘,’, ‘(’, ‘)’, ‘\\’.

## SUDOERS OPTIONS

sudo's behavior can be modified by Default_Entry lines, as explained earlier.  A list of all supported Defaults parameters, grouped by type, are listed below.

### Boolean Flags:

* noexec

  If set, all commands run via sudo will behave as if the NOEXEC tag has been set, unless overridden by an EXEC tag.  See the description of EXEC and NOEXEC as well as the Preventing shell escapes section at the end of this manual.  This flag is off by default.

* noninteractive_auth
  If set, authentication will be attempted even in non-interactive mode (when sudo's -n option is specified).  This allows authentication methods that don't require user interaction to succeed.  Authentication methods that require input from the user's terminal will still fail.  If disabled, authentication will not be attempted in non-interactive mode.  This flag is off by default.

* env_editor

  If set, visudo will use the value of the SUDO_EDITOR, VISUAL or EDITOR environment variables before falling back on the default editor list.  Note that visudo is typically run as root so this flag may allow a user with visudo privileges to run arbitrary commands as root without logging.  An alternative is to place a colon-separated list of “safe” editors int the editor setting.  visudo will then only use SUDO_EDITOR, VISUAL or EDITOR if they match a value specified in editor.  If the env_reset flag is enabled, the SUDO_EDITOR, VISUAL and/or EDITOR environment variables must be present in the env_keep list for the env_editor flag to function when visudo is invoked via sudo.  This flag is on by default.

* pwfeedback

  By default, sudo reads the password like most other Unix programs, by turning off echo until the user hits the return (or enter) key.  Some users become confused by this as it appears to them that sudo has hung at this point.  When pwfeedback is set, sudo will provide visual feedback when the user presses a key.  Note that this does have a security impact as an onlooker may be able to determine the length of the password being entered.  This flag is on by default.

* rootpw

  If set, sudo will prompt for the root password instead of the password of the invoking user when running a command or editing a file.  This flag is off by default.

* setenv

  Allow the user to set environment variables set via the command line that are not subject to the restrictions imposed by env_check, env_delete, or env_keep.  As such, only trusted users should be allowed to set variables in this manner.  This flag is off by default.

* targetpw

  If set, sudo will prompt for the password of the user specified by the -u option (defaults to root) instead of the password of the invoking user when running a command or editing a file. Note that this flag precludes the use of a user-ID not listed in the passwd database as an argument to the -u option. This flag is off by default.

* umask_override

  If set, sudo will set the umask as specified in the sudoers file without modification. This makes it possible to specify a umask in the sudoers file that is more permissive than the user's own umask. If umask_override is not set, sudo will set the umask to be the union of the user's umask and what is specified in sudoers. This flag is off by default.

* use_pty

  If set, and sudo is running in a terminal, the command will be run in a pseudo-terminal (even if no I/O logging is being done).  If the sudo process is not attached to a terminal, use_pty has no effect.

  A malicious program run under sudo may be capable of injecting commands into the user's terminal or running a background process that retains access to the user's terminal device even after the main program has finished executing.  By running the command in a separate pseudo-terminal, this attack is no longer possible.  This flag is on by default.

## Integers:

* passwd_tries

  The number of tries a user gets to enter his/her password before sudo logs the failure and exits.  The default is 3.

## Integers that can be used in a boolean context:

* timestamp_timeout

  Number of minutes that can elapse before sudo will ask for a passwd again.  The timeout may include a fractional component if minute granularity is insufficient, for example 2.5.  The default is 15.  Set this to 0 to always prompt for a password.

* umask

  File mode creation mask to use when running the command. Negate this option or set it to 0777 to prevent sudo from changing the umask. Unless the umask_override flag is set, the actual umask will be the union of the user's umask and the  value  of  the umask  setting, which defaults to 0022.  This guarantees that sudo never lowers the umask when running  a command.

  If umask is explicitly set, it will override any umask setting in PAM. If umask is not set, the umask specified by PAM will take precedence. The umask setting in PAM is not used for sudoedit, which does not create a new PAM session.

## Strings

* editor

  A colon (‘:’) separated list of editor path names used by **sudoedit** and **visudo**. For **sudoedit**, this list is used to find an editor when none of the SUDO_EDITOR, VISUAL or EDITOR environment variables are set to an editor that exists and is executable.  For **visudo**, it is used as a white list of allowed editors; **visudo** will choose the editor that matches the user's SUDO_EDITOR, VISUAL or EDITOR environment variable if possible, or the  first  editor in  the  list that exists and is executable if not. Unless invoked as **sudoedit**, sudo does not preserve the SUDO_EDITOR, VISUAL or EDITOR environment variables unless they are present in the **env_keep** list. The default on Linux is _/usr/bin/editor:/usr/bin/nano:/usr/bin/vi_. On FreeBSD the default is _/usr/bin/vi_.

## Strings that can be used in a boolean context:

* apparmor_profile

  The default AppArmor profile to transition into when executing a command. The default apparmor_profile can be overridden for individual sudoers entries by specifying the APPARMOR_PROFILE option. This option is only available when sudo-rs is built with AppArmor support. This option is not set by default.

* secure_path

  If set, sudo will use this value in place of the user's PATH environment variable.  This option can be used to reset the PATH to a known good value that contains directories for system administrator commands such as /usr/sbin. This option is not set by default.

## Lists that can be used in a boolean context:

* env_check

  Environment variables to be removed from the user's environment unless they are considered “safe”.  For all variables except TZ, “safe” means that the variable's value does not contain any ‘%’ or ‘/’ characters.  This can be used to guard against printf-style format vulnerabilities in poorly-written programs.  The TZ variable is considered unsafe if any of the following are true:

                       •  It consists of a fully-qualified path name, optionally prefixed with a colon (‘:’), that does not match the location of the zoneinfo directory.

                       •  It contains a .. path element.

                       •  It contains white space or non-printable characters.

                       •  It is longer than the value of PATH_MAX.

The argument may be a double-quoted, space-separated list or a single value without double-quotes.  The list can be replaced, added to, deleted from, or disabled by using the =, +=, -=, and ! operators respectively.  Regardless of whether the env_reset option is enabled or disabled, variables specified by env_check will be preserved in the environment if they pass the aforementioned check.  The global list of environment variables to check is displayed when sudo is run by root with the -V option.

* env_keep

  Environment variables to be preserved in the user's environment when the env_reset option is in effect.  This allows fine-grained control over the environment sudo-spawned processes will receive.  The argument may be a double-quoted, space-separated list or a single value without double-quotes.  The list can be replaced, added to, deleted from, or disabled by using the =, +=, -=, and ! operators respectively.  The global list of variables to keep is displayed when sudo is run by root with the -V option.

  Preserving the HOME environment variable has security implications since many programs use it when searching for configuration or data files.  Adding HOME to env_keep may enable a user to run unrestricted commands via sudo and is strongly discouraged. Users wishing to edit files with sudo should run **sudoedit** (or **sudo -e**) to get their accustomed editor configuration instead of invoking the editor directly.

## LOG FORMAT

sudo-rs logs events via syslog(3).

## FILES

     /etc/sudoers-rs           List of who can run what (for co-existence of sudo-rs and Todd Miller's sudo)

     /etc/sudoers              List of who can run what (sudo-compatible)

     /run/sudo/ts              Directory containing timestamps for the sudoers security policy

## SECURITY NOTES

### Limitations of the ‘!’ operator

It is generally not effective to “subtract” commands from ALL using the ‘!’ operator.  A user can trivially circumvent this by copying the desired command to a different name and then executing that.  For example:

     bill    ALL = ALL, !SU, !SHELLS

Doesn't really prevent bill from running the commands listed in SU or SHELLS since he can simply copy those commands to a different name, or use a shell escape from an editor or other program.  Therefore, these kind of restrictions should be considered advisory at best (and reinforced by policy).

In general, if a user has sudo ALL there is nothing to prevent them from creating their own program that gives them a root shell (or making their own copy of a shell) regardless of any ‘!’ elements in the user specification.

### Security implications of `fast_glob`

sudo-rs uses `fast_glob, which further means it is not possible to reliably negate commands where the path name includes globbing (aka wildcard) characters.  This is because the Rust library's fnmatch function cannot resolve relative paths.  While this is typically only an inconvenience for rules that grant privileges, it can result in a security issue for rules that subtract or revoke privileges.

For example, given the following sudoers file entry:

     john    ALL = /usr/bin/passwd [a-zA-Z0-9]*, /usr/bin/chsh [a-zA-Z0-9]*,\
                   /usr/bin/chfn [a-zA-Z0-9]*, !/usr/bin/* root

User john can still run /usr/bin/passwd root if fast_glob is enabled by changing to /usr/bin and running ./passwd root instead.

### Preventing shell escapes

Once sudo executes a program, that program is free to do whatever it pleases, including run other programs.  This can be a security issue since it is not uncommon for a program to allow shell escapes, which lets a user bypass sudo's access control and logging.  Common programs that permit shell escapes include shells (obviously), editors, paginators (such as *less*), mail, and terminal programs.

On Linux, sudo-rs has sudo's **noexec** functionality, based on a seccomp() filter. Programs that are run in **noexec** mode cannot run other programs. The implementation
in sudo-rs is different than in Todd Miller's sudo, and should also work on statically linked binaries.

Note that restricting shell escapes is not a panacea. Programs running as root are still capable of many potentially hazardous operations (such as changing or overwriting files) that could lead to unintended privilege escalation. NOEXEC is also not a protection against malicious programs. It doesn't prevent mapping memory as executable, nor does it protect against future syscalls that can do an exec() like the proposed `io_uring` exec feature in Linux. And it also doesn't protect against honest programs that intentionally or not allow the user to write to /proc/self/mem for the same reasons as that it doesn't protect against malicious programs.
You should always try out if **noexec** indeed prevents shell escapes for the programs it is intended to be used with.

### Timestamp file checks

sudo-rs will check the ownership of its timestamp directory (/run/sudo/ts by default) and ignore the directory's contents if it is not owned by root or if it is writable by a user other than root.

While the timestamp directory should be cleared at reboot time, to avoid potential problems, sudo-rs will ignore timestamp files that date from before the machine booted on systems where the boot time is available.

Some systems with graphical desktop environments allow unprivileged users to change the system clock.  Since sudo-rs relies on the system clock for timestamp validation, it may be possible on such systems for a user to run sudo for longer than *timestamp_timeout* by setting the clock back.  To combat this, `sudo-rs` uses a monotonic clock (which never moves backwards) for its timestamps if the system supports it.  sudo-rs will not honor timestamps set far in the future.

## SEE ALSO

su(1), fnmatch(3), glob(3), sudo(8), visudo(8)

## CAVEATS

The sudoers file should always be edited by the visudo utility which locks the file and checks for syntax errors.  If sudoers contains syntax errors, you may lock yourself out of being able to use sudo.

## BUGS

If you feel you have found a bug in sudo-rs, please submit a bug report at https://github.com/trifectatechfoundation/sudo-rs/issues/

# AUTHORS

This man page is a modified version of the sudoers(5) documentation written by Todd Miller; see https://www.sudo.ws/ for the original.

## DISCLAIMER

sudo-rs is provided “AS IS” and any express or implied warranties, including, but not limited to, the implied warranties of merchantability and fitness for a particular purpose are disclaimed.
