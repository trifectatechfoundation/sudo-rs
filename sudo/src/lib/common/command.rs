use std::path::{Path, PathBuf};

use super::{resolve::resolve_path, Error};

#[derive(Debug, Default)]
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

//checks whether the Path is actually describing a qualified path (i.e. contains "/")
//or just specifying the name of a file (in which case we are going to resolve it via PATH)
fn is_qualified(path: impl AsRef<Path>) -> bool {
    path.as_ref().parent() != Some(Path::new(""))
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

            // FIXME: we leak information here since we throw an error if a file does not exists
            if !is_qualified(&command) {
                command =
                    resolve_path(&command, path).ok_or_else(|| Error::InvalidCommand(command))?
            };
        }

        Ok(CommandAndArguments { command, arguments })
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

    #[test]
    fn qualified_paths() {
        use super::is_qualified;
        assert!(is_qualified("foo/bar"));
        assert!(is_qualified("a/b/bar"));
        assert!(is_qualified("a/b//bar"));
        assert!(is_qualified("/bar"));
        assert!(is_qualified("/bar/"));
        assert!(is_qualified("/bar/foo/"));
        assert!(is_qualified("/"));
        assert!(is_qualified("")); // don't try to resolve ""
        assert!(!is_qualified("bar"));
    }
}
