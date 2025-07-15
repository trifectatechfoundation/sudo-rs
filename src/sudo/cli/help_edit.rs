pub const USAGE_MSG: &str = "\
usage: sudoedit -h | -V
usage: sudoedit [-BknS] [-p prompt] [-g group] [-u user] file ...";

const DESCRIPTOR: &str = "sudo - edit files as another user";

const HELP_MSG: &str = "Options:
Options:
  -B, --bell                    ring bell when prompting
  -g, --group=group             run command as the specified group name or ID
  -h, --help                    display help message and exit
  -k, --reset-timestamp         invalidate timestamp file
  -n, --non-interactive         non-interactive mode, no prompts are used
  -p, --prompt=prompt           use the specified password prompt
  -S, --stdin                   read password from standard input
  -u, --user=user               run command (or edit file) as specified user
                                name or ID
  -V, --version                 display version information and exit
  --                            stop processing command line arguments";

pub fn long_help_message() -> String {
    format!("{DESCRIPTOR}\n{USAGE_MSG}\n{HELP_MSG}")
}
