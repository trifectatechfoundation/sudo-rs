use std::{ffi::OsString, path::PathBuf};

use crate::{resolve::resolve_path, Error};

#[derive(Debug)]
pub struct CommandAndArguments {
    pub command: PathBuf,
    pub arguments: Vec<OsString>,
}

impl CommandAndArguments {
    pub fn try_from_args(external_args: &[String], path: &str) -> Result<Self, Error> {
        let mut iter = external_args.iter();
        let command = iter
            .next()
            .ok_or(Error::InvalidCommand(String::new()))?
            .to_string();

        // resolve the binary if the path is not absolute
        let command = if command.starts_with('/') {
            PathBuf::from(command)
        } else {
            resolve_path(&PathBuf::from(&command), path)
                .ok_or_else(|| Error::InvalidCommand(command))?
        };

        Ok(CommandAndArguments {
            command,
            arguments: iter.map(|v| v.into()).collect(),
        })
    }
}
