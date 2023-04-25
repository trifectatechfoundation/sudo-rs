use std::path::PathBuf;

use crate::{resolve::resolve_path, Error};

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CommandAndArguments {
    pub command: PathBuf,
    pub arguments: Vec<String>,
}

// when -i and -s are used, the arguments given to sudo are escaped "except for alphanumerics, underscores, hyphens, and dollar signs."
fn escaped(arguments: Vec<String>) -> String {
    arguments
        .into_iter()
        .map(|arg| {
            arg.chars()
                .map(|c| match c {
                    '_' | '-' | '$' => c.to_string(),
                    c if c.is_alphanumeric() => c.to_string(),
                    _ => ['\\', c].iter().collect(),
                })
                .collect()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

impl CommandAndArguments {
    pub fn try_from_args(
        shell: Option<PathBuf>,
        mut arguments: Vec<String>,
        path: &str,
    ) -> Result<Self, Error> {
        let mut command;
        if let Some(chosen_shell) = shell {
            command = chosen_shell;
            if !arguments.is_empty() {
                arguments = vec!["-c".to_string(), escaped(arguments)]
            }
        } else {
            command = arguments
                .get(0)
                .ok_or_else(|| Error::InvalidCommand(PathBuf::new()))?
                .into();
            arguments.remove(0);

            // resolve the binary if the path is not absolute
            if command.is_relative() {
                command =
                    resolve_path(&command, path).ok_or_else(|| Error::InvalidCommand(command))?
            };
        }

        Ok(CommandAndArguments { command, arguments })
    }
}

impl ToString for CommandAndArguments {
    fn to_string(&self) -> String {
        format!(
            "{}{}{}",
            self.command.display(),
            if self.arguments.is_empty() { "" } else { " " },
            self.arguments.join(" ")
        )
    }
}

#[cfg(test)]
mod test {
    use super::{escaped, CommandAndArguments};

    #[test]
    fn test_escaped() {
        let test = |src: &[&str], target: &str| {
            assert_eq!(
                &escaped(src.iter().map(|s| s.to_string()).collect()),
                target
            );
        };
        test(&["a", "b", "c"], "a b c");
        test(&["a", "b c"], "a b\\ c");
        test(&["a", "b-c"], "a b-c");
        test(&["a", "b#c"], "a b\\#c");
        test(&["1 2 3"], "1\\ 2\\ 3");
        test(&["! @ $"], "\\!\\ \\@\\ $");
    }

    #[test]
    fn test_build_command_and_args() {
        assert_eq!(
            CommandAndArguments::try_from_args(
                None,
                vec!["/bin/ls".into(), "hello".into()],
                "/bin"
            )
            .unwrap(),
            CommandAndArguments {
                command: "/bin/ls".into(),
                arguments: vec!["hello".into()]
            }
        );

        assert_eq!(
            CommandAndArguments::try_from_args(None, vec!["ls".into(), "hello".into()], "/bin")
                .unwrap(),
            CommandAndArguments {
                command: "/bin/ls".into(),
                arguments: vec!["hello".into()]
            }
        );
        assert_eq!(
            CommandAndArguments::try_from_args(
                Some("shell".into()),
                vec!["ls".into(), "hello".into()],
                "/bin"
            )
            .unwrap(),
            CommandAndArguments {
                command: "shell".into(),
                arguments: vec!["-c".into(), "ls hello".into()]
            }
        );
    }
}
