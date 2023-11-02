#![forbid(unsafe_code)]
use std::{collections::HashMap, ffi::OsString};

pub use command::CommandAndArguments;
pub use context::Context;
pub use error::Error;
pub use path::SudoPath;
pub use string::SudoString;

pub mod bin_serde;
pub mod command;
pub mod context;
pub mod error;
mod path;
pub mod resolve;
mod string;

pub type Environment = HashMap<OsString, OsString>;

// Hardened enum values used for critical enums to mitigate attacks like Rowhammer.
// See for example https://arxiv.org/pdf/2309.02545.pdf
// The values are copied from https://github.com/sudo-project/sudo/commit/7873f8334c8d31031f8cfa83bd97ac6029309e4f#diff-b8ac7ab4c3c4a75aed0bb5f7c5fd38b9ea6c81b7557f775e46c6f8aa115e02cd
pub const HARDENED_ENUM_VALUE_0: u32 = 0x52a2925; // 0101001010100010100100100101
pub const HARDENED_ENUM_VALUE_1: u32 = 0xad5d6da; // 1010110101011101011011011010
pub const HARDENED_ENUM_VALUE_2: u32 = 0x69d61fc8; // 1101001110101100001111111001000
pub const HARDENED_ENUM_VALUE_3: u32 = 0x1629e037; // 0010110001010011110000000110111
pub const HARDENED_ENUM_VALUE_4: u32 = 0x1fc8d3ac; // 11111110010001101001110101100
