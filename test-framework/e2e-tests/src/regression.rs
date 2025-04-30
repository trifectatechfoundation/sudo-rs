use sudo_test::{Command, Env, TextFile, User};

#[test]
fn syslog_writer_should_not_hang() {
    let env = Env(TextFile("ALL ALL=(ALL:ALL) NOPASSWD: ALL").chmod("644")).build();

    let stdout = Command::new("sudo")
        .args(["env", "CC=clang-18", "CXX=clang++-18", "FOO=\"........................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................\"", "whoami"])
        .output(&env)
        .stdout();

    assert_eq!(stdout, "root");
}

#[test]
fn no_permissions_should_not_violate_io_safety() {
    let env = Env(TextFile("ALL ALL=(ALL:ALL) NOPASSWD: ALL").chmod("644"))
        .file("/bin/foo", "#!/bin/sh") // File not executable
        .build();

    let output = Command::new("sudo").arg("/bin/foo").output(&env);

    assert!(!output.status().success());

    let stderr = output.stderr();
    assert!(!stderr.contains("IO Safety violation"), "{stderr}");

    assert_eq!(
        stderr,
        "sudo-rs: cannot execute '/usr/bin/foo': Permission denied (os error 13)"
    );
}

#[test]
fn user_without_permissions_cannot_distinguish_file_existence() {
    // run test for users both without any permissions and limited permissions that
    // exclude access to the directory of interest
    for sudoers in ["", "ALL ALL=/bin/true"] {
        let env = Env(sudoers)
            // ordinary users have no access to the directory
            .directory(sudo_test::Directory("/secret").chmod("777"))
            // this should be irrespective of file permissions
            .file("/secret/txt", TextFile("").chmod("000"))
            .file("/secret/foo", TextFile("#! /bin/sh").chmod("777"))
            .user(User("user").password("password"))
            .build();

        for bait in ["/secret/txt", "/secret/foo"] {
            let output = Command::new("sudo")
                .args(["-S", "-l", bait])
                .as_user("user")
                .stdin("password")
                .output(&env);
            assert!(!output.status().success());
            let response_1 = output.stderr().replace(bait, "<file>");

            let output = Command::new("sudo")
                .args(["-S", "-l", "/secret/missing"])
                .as_user("user")
                .stdin("password")
                .output(&env);
            assert!(!output.status().success());
            let response_2 = output.stderr().replace("/secret/missing", "<file>");

            dbg!(sudoers);
            dbg!(bait);
            dbg!(&response_1);
            assert_eq!(response_1, response_2);
        }
    }
}
