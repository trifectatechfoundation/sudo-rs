#![forbid(unsafe_code)]
use std::{collections::HashMap, ffi::OsString};

pub use command::CommandAndArguments;
pub use context::Context;
pub use error::Error;

pub mod command;
pub mod context;
pub mod error;
pub mod resolve;

pub type Environment = HashMap<OsString, OsString>;
