// the `std::print` macros panic on any IO error. these are non-panicking alternatives
macro_rules! println_ignore_io_error {
    ($($tt:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), $($tt)*);
    }}
}

macro_rules! eprintln_ignore_io_error {
    ($($tt:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), $($tt)*);
    }}
}

// catch unintentional uses of `print*` macros with the test suite
#[allow(unused_macros)]
#[cfg(debug_assertions)]
macro_rules! eprintln {
    ($($tt:tt)*) => {
        compiler_error!("do not use `eprintln!`; use the `write!` macro instead")
    };
}

#[allow(unused_macros)]
#[cfg(debug_assertions)]
macro_rules! eprint {
    ($($tt:tt)*) => {
        compiler_error!("do not use `eprint!`; use the `write!` macro instead")
    };
}

#[allow(unused_macros)]
#[cfg(debug_assertions)]
macro_rules! println {
    ($($tt:tt)*) => {
        compiler_error!("do not use `println!`; use the `write!` macro instead")
    };
}

#[allow(unused_macros)]
#[cfg(debug_assertions)]
macro_rules! print {
    ($($tt:tt)*) => {
        compiler_error!("do not use `print!`; use the `write!` macro instead")
    };
}
