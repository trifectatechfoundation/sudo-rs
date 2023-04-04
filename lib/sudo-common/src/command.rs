use std::path::PathBuf;

use crate::{resolve::resolve_path, Error};

#[derive(Debug)]
pub struct CommandAndArguments<'a> {
    pub command: PathBuf,
    pub arguments: Vec<&'a str>,
}

impl<'a> CommandAndArguments<'a> {
    pub fn try_from_args(external_args: &'a [String], path: &str) -> Result<Self, Error> {
        let mut iter = external_args.iter();
        let command = iter
            .next()
            .ok_or(Error::InvalidCommand(String::new()))?
            .to_string();

        // resolve the binary if the path is not absolute
        let command = if command.starts_with('/') {
            PathBuf::from(command)
        } else {
            // TODO: use value of secure_path setting to possible override current path
            // FIXME: propagating the error is a possible security problem since it leaks information before any permission check is done
            resolve_path(&PathBuf::from(&command), path)
                .ok_or_else(|| Error::InvalidCommand(command))?
        };

        Ok(CommandAndArguments {
            command,
            arguments: iter.map(|v| v.as_str()).collect(),
        })
    }
}
