use std::path::PathBuf;

use crate::{resolve::resolve_path, Error};

#[derive(Debug)]
pub struct CommandAndArguments {
    pub command: PathBuf,
    pub arguments: Vec<String>,
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
                arguments.insert(0, "-c".to_string());
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
