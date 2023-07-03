#![cfg(test)]

mod su {

    mod signal_handling;
}

type Error = Box<dyn std::error::Error>;
type Result<T> = core::result::Result<T, Error>;

const USERNAME: &str = "ferris";
