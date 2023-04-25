use std::path::PathBuf;
use sudo_cli::SudoOptions;
use sudo_system::{hostname, Group, Process, User};

use crate::{
    command::CommandAndArguments,
    resolve::{resolve_current_user, resolve_launch_and_shell, resolve_target_user_and_group},
    Error,
};

#[derive(Debug)]
pub struct Context {
    // cli options
    pub preserve_env_list: Vec<String>,
    pub set_home: bool,
    pub launch: LaunchType,
    pub chdir: Option<PathBuf>,
    pub command: CommandAndArguments,
    pub target_user: User,
    pub target_group: Group,
    pub stdin: bool,
    // system
    pub hostname: String,
    pub path: String,
    pub current_user: User,
    pub process: Process,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LaunchType {
    Direct,
    Shell,
    Login,
}

impl Context {
    pub fn build_from_options(sudo_options: SudoOptions, path: String) -> Result<Context, Error> {
        let hostname = hostname();
        let current_user = resolve_current_user()?;
        let (target_user, target_group) =
            resolve_target_user_and_group(&sudo_options.user, &sudo_options.group, &current_user)?;
        let (launch, shell) = resolve_launch_and_shell(&sudo_options, &current_user, &target_user);
        let command = CommandAndArguments::try_from_args(shell, sudo_options.external_args, &path)?;

        Ok(Context {
            hostname,
            path,
            command,
            current_user,
            target_user,
            target_group,
            set_home: sudo_options.set_home,
            preserve_env_list: sudo_options.preserve_env_list,
            launch,
            chdir: sudo_options.directory,
            stdin: sudo_options.stdin,
            process: sudo_system::Process::new(),
        })
    }

    const SUMMARY_SEPERATOR: &str = " : ";
    const ATTRIBUTE_SEPERATOR: &str = " ; ";

    pub fn get_summary(&self, notification: Option<&str>) -> String {
        let mut summary: Vec<&str> = vec![&self.hostname, &self.current_user.name];

        if let Some(n) = notification {
            summary.push(n);
        }

        let mut attributes: Vec<(&str, String)> = vec![];

        if let Some(cwd) = self
            .chdir
            .as_ref()
            .or(std::env::current_dir().as_ref().ok())
        {
            attributes.push(("CWD", cwd.display().to_string()));
        }

        attributes.push(("USER", self.target_user.name.clone()));
        attributes.push(("COMMAND", self.command.to_string()));

        attributes
            .into_iter()
            .fold(summary.join(Context::SUMMARY_SEPERATOR), |acc, (k, v)| {
                format!("{acc}{}{k}={v}", Context::ATTRIBUTE_SEPERATOR)
            })
    }
}

#[cfg(test)]
pub mod tests {
    use crate::CommandAndArguments;

    use super::{Context, LaunchType};
    use std::collections::HashMap;
    use sudo_cli::SudoOptions;
    use sudo_system::{Group, User};

    pub fn create_test_context<'a>(sudo_options: &'a SudoOptions) -> Context {
        let path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string();
        let command =
            CommandAndArguments::try_from_args(None, sudo_options.external_args.clone(), &path)
                .unwrap();

        let current_user = User {
            uid: 1000,
            gid: 1000,
            name: "test".to_string(),
            gecos: String::new(),
            home: "/home/test".into(),
            shell: "/bin/sh".into(),
            passwd: String::new(),
            groups: vec![],
        };

        let current_group = Group {
            gid: 1000,
            name: "test".to_string(),
            passwd: String::new(),
            members: Vec::new(),
        };

        let root_user = User {
            uid: 0,
            gid: 0,
            name: "root".to_string(),
            gecos: String::new(),
            home: "/root".into(),
            shell: "/bin/bash".into(),
            passwd: String::new(),
            groups: vec![],
        };

        let root_group = Group {
            gid: 0,
            name: "root".to_string(),
            passwd: String::new(),
            members: Vec::new(),
        };

        Context {
            hostname: "test-host".to_string(),
            command,
            current_user: current_user.clone(),
            target_user: if sudo_options.user.as_deref() == Some("test") {
                current_user
            } else {
                root_user
            },
            target_group: if sudo_options.user.as_deref() == Some("test") {
                current_group
            } else {
                root_group
            },
            set_home: sudo_options.set_home,
            preserve_env_list: sudo_options.preserve_env_list.clone(),
            path,
            launch: LaunchType::Direct,
            chdir: sudo_options.directory.clone(),
            stdin: sudo_options.stdin,
            process: sudo_system::Process::new(),
        }
    }

    #[test]
    fn test_build_context() {
        let options = SudoOptions::try_parse_from(["sudo", "echo", "hello"]).unwrap();
        let path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
        let context = Context::build_from_options(options, path.to_string()).unwrap();

        let mut target_environment = HashMap::new();
        target_environment.insert("SUDO_USER".to_string(), context.current_user.name.clone());

        assert_eq!(context.command.command.to_str().unwrap(), "/usr/bin/echo");
        assert_eq!(context.command.arguments, ["hello"]);
        assert_eq!(context.hostname, sudo_system::hostname());
        assert_eq!(context.target_user.uid, 0);
    }

    #[test]
    fn test_summary() {
        let options =
            SudoOptions::try_parse_from(["sudo", "--chdir", "/root", "echo", "foo"]).unwrap();
        let context = create_test_context(&options);

        assert_eq!(
            context.get_summary(Some("hello")),
            "test-host : test : hello ; CWD=/root ; USER=root ; COMMAND=/usr/bin/echo foo"
        );

        let options = SudoOptions::try_parse_from(["sudo", "--chdir", "/home/test", "ls"]).unwrap();
        let context = create_test_context(&options);

        assert_eq!(
            context.get_summary(None),
            "test-host : test ; CWD=/home/test ; USER=root ; COMMAND=/usr/bin/ls"
        );
    }
}
