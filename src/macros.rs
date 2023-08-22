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
        panic!("do not use `eprintln!`")
    };
}

#[allow(unused_macros)]
#[cfg(debug_assertions)]
macro_rules! eprint {
    ($($tt:tt)*) => {
        panic!("do not use `eprint!`")
    };
}

#[allow(unused_macros)]
#[cfg(debug_assertions)]
macro_rules! println {
    ($($tt:tt)*) => {
        panic!("do not use `println!`")
    };
}

#[allow(unused_macros)]
#[cfg(debug_assertions)]
macro_rules! print {
    ($($tt:tt)*) => {
        panic!("do not use `print!`")
    };
}
