pub const USAGE_MSG: &str = "\
usage: sudo -h | -K | -k | -V
usage: sudo -v [-knS] [-g group] [-u user]
usage: sudo -l [-knS] [-g group] [-U user] [-u user] [command [arg ...]]
usage: sudo [-knS] [-D directory] [-g group] [-u user] [-i | -s] [command [arg ...]]
usage: sudo -e [-knS] [-D directory] [-g group] [-u user] file ...";

const DESCRIPTOR: &str = "sudo - run commands as another user";

const HELP_MSG: &str = "Options:
  -D, --chdir=directory         change the working directory before running command
  -g, --group=group             run command as the specified group name or ID
  -h, --help                    display help message and exit
  -i, --login                   run login shell as the target user; a command may also be specified
  -K, --remove-timestamp        remove timestamp file completely
  -k, --reset-timestamp         invalidate timestamp file
  -l, --list                    list user's privileges or check a specific command; use twice for longer format
  -n, --non-interactive         non-interactive mode, no prompts are used
  -S, --stdin                   read password from standard input
  -s, --shell                   run shell as the target user; a command may also be specified
  -U, --other-user=user         in list mode, display privileges for user
  -u, --user=user               run command (or edit file) as specified user name or ID
  -V, --version                 display version information and exit
  -v, --validate                update user's timestamp without running a command
  --                            stop processing command line arguments";

pub fn long_help_message() -> String {
    format!("{DESCRIPTOR}\n{USAGE_MSG}\n{HELP_MSG}")
}
