use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser, Clone)]
#[clap(
    name = "sudo-rs",
    about = "sudo - execute a command as another user",
    version,
    // disable_version_flag = true,
    disable_help_flag = true,
    trailing_var_arg = true,
    override_usage = "usage: sudo -h | -K | -k | -V
    usage: sudo -v [-AknS] [-g group] [-h host] [-p prompt] [-u user]
    usage: sudo -l [-AknS] [-g group] [-h host] [-p prompt] [-U user] [-u user] [command]
    usage: sudo [-AbEHknPS] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R directory] [-T timeout] [-u user] [VAR=value] [-i|-s] [<command>]
    usage: sudo -e [-AknS] [-C num] [-D directory] [-g group] [-h host] [-p prompt] [-R directory] [-T timeout] [-u user] file ...",
)]
struct Cli {
    #[arg(
        long,
        short = 'A',
        help = "use a helper program for password prompting",
        action
    )]
    askpass: bool,
    #[arg(short = 'b', long, help = "run command in the background", action)]
    background: bool,
    #[arg(short = 'B', long, help = "ring bell when prompting", action)]
    bell: bool,
    #[arg(
        short = 'C',
        long = "close-from",
        help = "close all file descriptors >= num"
    )]
    num: Option<i16>,
    #[arg(
        short = 'D',
        long = "chdir",
        help = "change the working directory before running command"
    )]
    directory: Option<PathBuf>,
    #[arg(long, help = "preserve specific environment variables", value_name = "list", value_delimiter=',', default_value = None, default_missing_value = "", require_equals = true, num_args = 0..)]
    preserve_env: Vec<String>,
    #[arg(short = 'E', help = "preserve user environment when running command")]
    short_preserve_env: bool,
    #[arg(
        short = 'e',
        long,
        help = "edit files instead of running a command",
        action
    )]
    edit: bool,
    #[arg(
        short = 'g',
        long = "group",
        help = "run command as the specified group name or ID"
    )]
    group: Option<String>,
    #[arg(
        short = 'H',
        long = "set-home",
        help = "set HOME variable to target user's home dir",
        action
    )]
    set_home: bool,
    #[arg(
        short = 'i',
        long,
        help = "run login shell as the target user; a command may also be specified",
        action,
        conflicts_with("shell")
    )]
    login: bool,
    #[arg(
        short = 'K',
        long = "remove-timestamp",
        help = "remove timestamp file completely",
        action,
        conflicts_with("reset_timestamp"),
        conflicts_with("version")
    )]
    remove_timestamp: bool,
    #[arg(
        short = 'k',
        long = "reset-timestamp",
        help = "invalidate timestamp file",
        action,
        conflicts_with("remove_timestamp"),
        conflicts_with("version")
    )]
    reset_timestamp: bool,
    #[arg(
        short,
        long,
        help = "list user's privileges or check a specific command; use twice for longer format",
        action
    )]
    list: bool,
    #[arg(
        short = 'n',
        long = "non-interactive",
        help = "non-interactive mode, no prompts are used",
        action
    )]
    non_interactive: bool,
    #[arg(
        short = 'P',
        long = "preserve-groups",
        help = "preserve group vector instead of setting to target's",
        action
    )]
    preserve_groups: bool,
    #[arg(
        short = 'p',
        long = "prompt",
        help = "use the specified password prompt"
    )]
    prompt: Option<String>,
    #[arg(
        short = 'R',
        long = "chroot",
        help = "change the root directory before running command",
        value_name = "directory"
    )]
    chroot: Option<PathBuf>,
    #[arg(short = 'S', long, help = "read password from standard input", action)]
    stdin: bool,
    #[arg(
        short = 's',
        long,
        help = "run shell as the target user; a command may also be specified",
        action
    )]
    shell: bool,
    #[arg(
        short = 'T',
        long = "command-timeout",
        help = "terminate command after the specified time limit",
        value_name = "timeout"
    )]
    command_timeout: Option<String>, // To Do: This is the wrong type. Which one is correct?
    #[arg(
        short = 'U',
        long = "other-user",
        help = "in list mode, display privileges for user",
        value_name = "user"
    )]
    other_user: Option<String>,
    #[arg(
        short = 'u',
        long = "user",
        help = "run command (or edit file) as specified user name or ID"
    )]
    user: Option<String>,
    // #[arg(short = 'V', long = "version", help = "display version information and exit!", action = ArgAction::Version, conflicts_with("host"), conflicts_with("remove_timestamp"), conflicts_with("reset_timestamp"))]
    // version: bool,
    #[arg(
        short = 'v',
        long,
        help = "update user's timestamp without running a command",
        action
    )]
    validate: bool,
    #[arg(short = 'h', value_name = "host", default_value = None, default_missing_value = "", require_equals = true, num_args = 0..=1)]
    host_or_help: Option<String>,
    #[arg(long, value_name = "host")]
    host: Option<String>,
    // this is a hack to make help show up for `--`, which wouldn't be allowed as a flag in clap.
    // Ignore value of `stop_processing_args`.
    #[arg(long = " ", help = "stop processing command line arguments", action)]
    stop_processing_args: bool,
    // Arguments passed straight through, either seperated by -- or just trailing.
    #[arg(hide = true)]
    external_args: Vec<String>,
}

#[derive(Debug)]
pub struct SudoOptions {
    pub askpass: bool,
    pub background: bool,
    pub bell: bool,
    pub num: Option<i16>,
    pub directory: Option<PathBuf>,
    // This is what OGsudo calls `--preserve-env=list`
    pub preserve_env_list: Vec<String>,
    // This is what OGsudo calls `-E, --preserve-env`
    pub preserve_env: bool,
    // Only one of the -e, -h, -i, -K, -l, -s, -v or -V options may be specified
    pub edit: bool,
    pub group: Option<String>,
    pub set_home: bool,
    // #[arg(long, help = "display help message and exit!", action = ArgAction::Help)]
    // pub help: bool, // TO DO: help as well as host are supposed to have short 'h'???
    // #[arg(short = 'h', long = "host", help = "run command on host (if supported by plugin)")]
    // pub host: Option<String>,
    // possible to do the same way as preserve_env
    pub login: bool,
    pub remove_timestamp: bool,
    pub reset_timestamp: bool,
    pub list: bool,
    pub non_interactive: bool,
    pub preserve_groups: bool,
    pub prompt: Option<String>,
    pub chroot: Option<PathBuf>,
    pub stdin: bool,
    pub shell: bool,
    pub command_timeout: Option<String>,
    pub other_user: Option<String>,
    pub user: Option<String>,
    // pub version: bool,
    pub validate: bool,
    pub help: bool,
    pub host: Option<String>,
    // this is a hack to make help show up for `--`, which wouldn't be allowed as a flag in clap.
    // Ignore value of `stop_processing_args`.
    // pub stop_processing_args: bool,
    // Arguments passed straight through, either seperated by -- or just trailing.
    pub external_args: Vec<String>,
    pub env_var_list: Vec<(String, String)>,
}

impl From<Cli> for SudoOptions {
    fn from(command: Cli) -> Self {
        let is_help = command.host_or_help.as_deref() == Some("");
        let host = match command.host {
            Some(host) => {
                if !is_help {
                    todo!("Both `-h=<HOST>` and `--help=<HOST>` are being used")
                }
                Some(host)
            }
            None => {
                if !is_help {
                    command.host_or_help
                } else {
                    None
                }
            }
        };

        // This lets us know if the user passed `--preserve-env` with no args
        let preserve_env_no_args = command.preserve_env.iter().any(String::is_empty);

        Self {
            preserve_env: command.short_preserve_env || preserve_env_no_args,
            preserve_env_list: {
                // Filter any empty item from the list as this means that the user passed
                // `--preserve-env` with no args which is not relevant for this list.
                command
                    .preserve_env
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect()
            },
            askpass: command.askpass,
            background: command.background,
            bell: command.bell,
            num: command.num,
            directory: command.directory,
            edit: command.edit,
            group: command.group,
            set_home: command.set_home,
            login: command.login,
            remove_timestamp: command.remove_timestamp,
            reset_timestamp: command.reset_timestamp,
            list: command.list,
            non_interactive: command.non_interactive,
            preserve_groups: command.preserve_groups,
            prompt: command.prompt,
            chroot: command.chroot,
            stdin: command.stdin,
            shell: command.shell,
            command_timeout: command.command_timeout,
            other_user: command.other_user,
            user: command.user,
            validate: command.validate,
            help: is_help,
            host,
            external_args: command.external_args,
            env_var_list: Default::default(),
        }
    }
}

impl SudoOptions {
    pub fn parse() -> Self {
        let mut env_var_list = Vec::new();
        let mut remaining_args = Vec::new();

        let mut args = std::env::args();

        while let Some(arg) = args.next() {
            // If we found `--` we know that the remaining arguments are not env variable
            // definitions.
            if arg == "--" {
                remaining_args.push(arg);
                remaining_args.extend(args);
                break;
            }

            if let Some((name, value)) = arg.split_once('=').and_then(|(name, value)| {
                name.chars()
                    .all(|c| c.is_alphanumeric() || c == '_')
                    .then_some((name, value))
            }) {
                env_var_list.push((name.to_owned(), value.to_owned()));
            } else {
                remaining_args.push(arg);
            }
        }

        let mut opts: SudoOptions = Cli::parse_from(remaining_args).into();
        opts.env_var_list = env_var_list;

        opts
    }
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    Cli::command().debug_assert()
}
