#[cfg_attr(all(target_pointer_width = "64", target_os = "linux"), path = "x86_64_linux.rs")]
mod inner;

pub use inner::*;
