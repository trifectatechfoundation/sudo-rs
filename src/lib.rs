#[macro_use]
mod macros;
#[cfg(feature = "apparmor")]
pub(crate) mod apparmor;
pub(crate) mod common;
pub(crate) mod cutils;
pub(crate) mod defaults;
pub(crate) mod exec;
pub(crate) mod log;
pub(crate) mod pam;
pub(crate) mod sudoers;
pub(crate) mod system;

mod su;
mod sudo;
mod visudo;

pub use su::main as su_main;
pub use sudo::main as sudo_main;
pub use visudo::main as visudo_main;

#[cfg(feature = "do-not-use-all-features")]
compile_error!("Refusing to compile using 'cargo --all-features' --- please read the README");
