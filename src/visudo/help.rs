pub(crate) const USAGE_MSG: &str = "usage: visudo [-chqsV] [[-f] sudoers ]";

const DESCRIPTOR: &str = "visudo - safely edit the sudoers file";

const HELP_MSG: &str = "Options:
  -c, --check              check-only mode
  -f, --file=sudoers       specify sudoers file location
  -h, --help               display help message and exit
  -I, --no-includes        do not edit include files
  -q, --quiet              less verbose (quiet) syntax error messages
  -s, --strict             strict syntax checking
  -V, --version            display version information and exit
";

pub(crate) fn long_help_message() -> String {
    format!("{USAGE_MSG}\n\n{DESCRIPTOR}\n\n{HELP_MSG}")
}
