pub const USAGE_MSG: &str = "Usage: su [options] [-] [<user> [<argument>...]]";

const DESCRIPTOR: &str = "Change the effective user ID and group ID to that of <user>.
A mere - implies -l.  If <user> is not given, root is assumed.";

const HELP_MSG: &str = "Options:
-m, -p, --preserve-environment      do not reset environment variables
-w, --whitelist-environment <list>  don't reset specified variables

-g, --group <group>             specify the primary group
-G, --supp-group <group>        specify a supplemental group

-, -l, --login                  make the shell a login shell
-c, --command <command>         pass a single command to the shell with -c
-s, --shell <shell>             run <shell> if /etc/shells allows it
-P, --pty                       create a new pseudo-terminal

-h, --help                      display this help
-V, --version                   display version
";

pub fn long_help_message() -> String {
    format!("{USAGE_MSG}\n\n{DESCRIPTOR}\n\n{HELP_MSG}")
}
