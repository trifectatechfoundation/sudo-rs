#![cfg(test)]

mod pty;
mod regression;
mod su;

const USERNAME: &str = "ferris";

#[test]
fn sanity_check() {
    assert!(
        !sudo_test::is_original_sudo(),
        "you must set `SUDO_UNDER_TEST=ours` when running this test suite"
    );
}
