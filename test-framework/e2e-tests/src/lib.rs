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

#[test]
#[cfg(feature = "apparmor")]
fn dlopen_apparmor_ignores_ld_library_path() -> Result<(), Box<dyn std::error::Error>> {
    use sudo_test::{Command, Env};

    let env = Env("foo ALL=(ALL:ALL) APPARMOR_PROFILE=docker-default NOPASSWD: ALL")
        .file(
            "/tmp/crash_me.c",
            "#include <stdlib.h>

void __attribute__((constructor)) do_not_load() {
    abort();
}
",
        )
        .user("foo")
        .apparmor("unconfined")
        .build();

    Command::new("gcc")
        .args(["/tmp/crash_me.c", "-shared", "-o", "/tmp/libapparmor.so.1"])
        .output(&env)
        .assert_success();

    let output = Command::new("sh")
        .args([
            "-c",
            "LD_LIBRARY_PATH=/tmp sudo -s cat /proc/\\$\\$/attr/current",
        ])
        .as_user("foo")
        .output(&env);

    output.assert_success();
    assert_eq!(output.stdout(), "docker-default (enforce)");

    let output = Command::new("sh")
        .args(["-c", "LD_PRELOAD=/tmp/libapparmor.so.1 ls"])
        .output(&env);

    output.assert_exit_code(134); // SIGABRT
    assert_eq!(output.stderr(), "Aborted (core dumped)");

    Ok(())
}
