use sudo_test::{Command, Env};

// `Defaults runcwd=<dir>` runs the command in `<dir>` instead of the invoking
// user's working directory.
#[test]
fn sets_the_default_working_directory() {
    let env = Env("\
Defaults runcwd=/root
ALL ALL=(ALL:ALL) ALL")
    .build();

    let stdout = Command::new("sh")
        .args(["-c", "cd /; sudo pwd"])
        .output(&env)
        .stdout();

    assert_eq!("/root", stdout);
}

// `runcwd=*` only enables `--chdir`; on its own it keeps the invoking user's
// working directory.
#[test]
fn glob_keeps_the_invoking_directory() {
    let env = Env("\
Defaults runcwd=*
ALL ALL=(ALL:ALL) ALL")
    .build();

    let stdout = Command::new("sh")
        .args(["-c", "cd /tmp; sudo pwd"])
        .output(&env)
        .stdout();

    assert_eq!("/tmp", stdout);
}

// a per-command `CWD` takes precedence over the `runcwd` default.
#[test]
fn cwd_tag_overrides_runcwd() {
    let env = Env("\
Defaults runcwd=/root
ALL ALL=(ALL:ALL) CWD=/tmp ALL")
    .build();

    let stdout = Command::new("sh")
        .args(["-c", "cd /; sudo pwd"])
        .output(&env)
        .stdout();

    assert_eq!("/tmp", stdout);
}
