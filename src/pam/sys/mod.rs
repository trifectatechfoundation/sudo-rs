#[cfg_attr(
    all(target_pointer_width = "64", target_os = "linux"),
    path = "x86_64_linux.rs"
)]
#[cfg_attr(
    all(target_pointer_width = "32", target_os = "linux"),
    path = "i386_linux.rs"
)]
mod inner;

pub use inner::*;
