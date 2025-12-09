use crate::common::resolve::CurrentUser;
use crate::common::{CommandAndArguments, Context};
use crate::sudo::{
    cli::{SudoAction, SudoRunOptions},
    env::environment::{get_target_environment, Environment},
};
use crate::system::interface::{GroupId, UserId};
use crate::system::{Group, Hostname, User};
use std::collections::{HashMap, HashSet};

const TESTS: &str = "
> env
    FOO=BAR
    HOME=/home/test
    HOSTNAME=test-ubuntu
    LANG=en_US.UTF-8
    LANGUAGE=en_US.UTF-8
    LC_ALL=en_US.UTF-8
    LS_COLORS=cd=40;33;01:*.jpg=01;35:*.mp3=00;36:
    PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
    PWD=/home/test
    SHLVL=0
    TERM=xterm
    _=/usr/bin/sudo
> sudo env
    HOSTNAME=test-ubuntu
    LANG=en_US.UTF-8
    LANGUAGE=en_US.UTF-8
    LC_ALL=en_US.UTF-8
    LS_COLORS=cd=40;33;01:*.jpg=01;35:*.mp3=00;36:
    PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
    SHELL=/bin/bash
    SUDO_COMMAND=/usr/bin/env
    SUDO_GID=1000
    SUDO_UID=1000
    SUDO_USER=test
    SUDO_HOME=/home/test
    HOME=/root
    LOGNAME=root
    USER=root
    TERM=xterm
> sudo -u test env
    HOSTNAME=test-ubuntu
    LANG=en_US.UTF-8
    LANGUAGE=en_US.UTF-8
    LC_ALL=en_US.UTF-8
    LS_COLORS=cd=40;33;01:*.jpg=01;35:*.mp3=00;36:
    PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
    SHELL=/bin/sh
    SUDO_COMMAND=/usr/bin/env
    SUDO_GID=1000
    SUDO_UID=1000
    SUDO_USER=test
    SUDO_HOME=/home/test
    HOME=/home/test
    LOGNAME=test
    USER=test
    TERM=xterm
";

fn parse_env_commands(input: &str) -> Vec<(&str, Environment)> {
    input
        .trim()
        .split("> ")
        .filter(|l| !l.is_empty())
        .map(|e| {
            let (cmd, vars) = e.split_once('\n').unwrap();

            let vars = vars
                .lines()
                .map(|line| line.trim().split_once('=').unwrap())
                .map(|(k, v)| (k.into(), v.into()))
                .collect();

            (cmd, vars)
        })
        .collect()
}

fn create_test_context(sudo_options: SudoRunOptions) -> Context {
    let path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string();
    let command = CommandAndArguments::build_from_args(None, sudo_options.positional_args, &path);

    let current_user = CurrentUser::fake(User {
        uid: UserId::new(1000),
        gid: GroupId::new(1000),

        name: "test".into(),
        home: "/home/test".into(),
        shell: "/bin/sh".into(),
        groups: vec![],
    });

    let current_group = Group {
        gid: GroupId::new(1000),
        name: Some("test".to_string()),
    };

    let root_user = User {
        uid: UserId::ROOT,
        gid: GroupId::new(0),
        name: "root".into(),
        home: "/root".into(),
        shell: "/bin/bash".into(),
        groups: vec![],
    };

    let root_group = Group {
        gid: GroupId::new(0),
        name: Some("root".to_string()),
    };

    Context {
        hostname: Hostname::fake("test-ubuntu"),
        command,
        current_user: current_user.clone(),
        target_user: if sudo_options.user.as_deref() == Some("test") {
            current_user.into()
        } else {
            root_user
        },
        target_group: if sudo_options.user.as_deref() == Some("test") {
            current_group
        } else {
            root_group
        },
        launch: crate::common::context::LaunchType::Direct,
        chdir: sudo_options.chdir,
        askpass: sudo_options.askpass,
        stdin: sudo_options.stdin,
        prompt: sudo_options.prompt,
        non_interactive: sudo_options.non_interactive,
        use_session_records: false,
        bell: false,
        background: false,
        files_to_edit: vec![],
    }
}

fn environment_to_set(environment: Environment) -> HashSet<String> {
    HashSet::from_iter(
        environment
            .into_iter()
            .map(|(k, v)| format!("{}={}", k.to_str().unwrap(), v.to_str().unwrap())),
    )
}

#[test]
fn test_environment_variable_filtering() {
    let mut parts = parse_env_commands(TESTS);
    let initial_env = parts.remove(0).1;

    for (cmd, expected_env) in parts {
        let options = SudoAction::try_parse_from(cmd.split_whitespace())
            .unwrap()
            .try_into_run()
            .ok()
            .unwrap();
        let settings = crate::defaults::Settings::default();
        let context = create_test_context(options);
        let resulting_env = get_target_environment(
            initial_env.clone(),
            HashMap::new(),
            Vec::new(),
            &context,
            &crate::sudoers::Restrictions {
                env_keep: settings.env_keep(),
                env_check: settings.env_check(),
                path: settings.secure_path(),
                use_pty: true,
                chdir: crate::sudoers::DirChange::Strict(None),
                trust_environment: false,
                umask: crate::exec::Umask::Preserve,
                #[cfg(feature = "apparmor")]
                apparmor_profile: None,
                noexec: false,
            },
        )
        .unwrap();

        let resulting_env = environment_to_set(resulting_env);
        let expected_env = environment_to_set(expected_env);
        let mut diff = resulting_env
            .symmetric_difference(&expected_env)
            .collect::<Vec<_>>();

        diff.sort();

        assert!(
            diff.is_empty(),
            "\"{cmd}\" results in an environment mismatch:\n{diff:#?}",
        );
    }
}
