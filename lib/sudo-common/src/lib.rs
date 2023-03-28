#![forbid(unsafe_code)]
use std::collections::HashMap;

pub use command::CommandAndArguments;
pub use context::Context;
pub use error::Error;

pub mod command;
pub mod context;
pub mod error;
pub mod resolve;

pub type Environment = HashMap<String, String>;
