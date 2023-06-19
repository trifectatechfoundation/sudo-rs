pub const HELP_MSG: &str = "sudo - execute a command as another user

usage: sudo -h | -K | -k | -V
usage: sudo -v [-knS] [-g group] [-h host] [-u user]
usage: sudo -l [-knS] [-g group] [-h host] [-U user] [-u user] [command]
usage: sudo [-bEHknPS] [-D directory] [-g group] [-h host] [-R
            directory] [-u user] [VAR=value] [-i|-s] [<command>]
usage: sudo -e [-knS] [-D directory] [-g group] [-h host] [-R
            directory] [-u user] file ...

Options:
  -b, --background              run command in the background
  -D, --chdir=directory         change the working directory before running command
  -E, --preserve-env=list       preserve specific environment variables
  -e, --edit                    edit files instead of running a command
  -g, --group=group             run command as the specified group name or ID
  -H, --set-home                set HOME variable to target user's home dir
  -h, --help                    display help message and exit
  -h, --host=host               run command on host (if supported by plugin)
  -i, --login                   run login shell as the target user; a command may also be
                                specified
  -K, --remove-timestamp        remove timestamp file completely
  -k, --reset-timestamp         invalidate timestamp file
  -l, --list                    list user's privileges or check a specific command; use twice
                                for longer format
  -n, --non-interactive         non-interactive mode, no prompts are used
  -P, --preserve-groups         preserve group vector instead of setting to target's
  -R, --chroot=directory        change the root directory before running command
  -S, --stdin                   read password from standard input
  -s, --shell                   run shell as the target user; a command may also be specified
  -U, --other-user=user         in list mode, display privileges for user
  -u, --user=user               run command (or edit file) as specified user name or ID
  -V, --version                 display version information and exit
  -v, --validate                update user's timestamp without running a command
  --                            stop processing command line arguments";

pub const USAGE_MSG: &str = "usage: sudo -h | -K | -k | -V
usage: sudo -v [-knS] [-g group] [-h host] [-u user]
usage: sudo -l [-knS] [-g group] [-h host] [-U user] [-u user] [command]
usage: sudo [-bEHknPS] [-D directory] [-g group] [-h host] [-R directory] [-u user] [VAR=value] [-i|-s] [<command>]
usage: sudo -e [-knS] [-D directory] [-g group] [-h host] [-R directory] [-u user] file ...";
