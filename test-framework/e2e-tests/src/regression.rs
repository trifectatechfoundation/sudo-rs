use std::sync::mpsc;
use std::time::Duration;

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
        "sudo: cannot execute '/usr/bin/foo': Permission denied (os error 13)"
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

#[test]
fn correct_password_with_tab() {
    let username = "ferris";
    let password = "secure-pwd";
    let env = Env(format!("{username}    ALL=(ALL:ALL) ALL"))
        .user(User(username).password(password))
        .build();

    for i in 0..password.len() {
        let mut no_echo_password = password.to_owned();
        no_echo_password.insert(i, '\t');
        Command::new("sshpass")
            .args(["-p", &no_echo_password, "sudo", "true"])
            .as_user(username)
            .output(&env)
            .assert_success();
    }
}

#[test]
fn sigttou_in_foreground_does_not_deadlock_sudo() {
    let inner_sh = "\
for _ in 1 2 3 4; do
    kill -TTOU $$
done
echo did-not-deadlock
";

    let launch_pl = r#"use strict;
use warnings;
use POSIX ();

my $pid = fork();
die "fork failed: $!" unless defined $pid;
if ($pid == 0) {
    POSIX::setpgid(0, 0) or die "setpgid (child) failed: $!";
    exec("sh", "-c", "sudo sh /root/inner.sh | cat") or die "exec failed: $!";
}
POSIX::setpgid($pid, $pid);
eval { POSIX::tcsetpgrp(0, $pid) };

waitpid($pid, 0);
exit($? == 0 ? 0 : 1);
"#;

    let env = Env(TextFile("ALL ALL=(ALL:ALL) NOPASSWD: ALL").chmod("644"))
        .file("/root/inner.sh", TextFile(inner_sh).chmod("755"))
        .file("/root/launch.pl", TextFile(launch_pl).chmod("755"))
        .build();

    let child = Command::new("perl")
        .arg("/root/launch.pl")
        // a tty is required for sudo to run the command in a pty
        .tty(true)
        .spawn(&env);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait());
    });

    match rx.recv_timeout(Duration::from_secs(30)) {
        Ok(output) => {
            output.assert_success();
            assert!(
                output.stdout_unchecked().contains("did-not-deadlock"),
                "unexpected output: {:?}",
                output.stdout_unchecked()
            );
        }
        Err(_) => panic!(
            "sudo deadlocked after the command was stopped twice by SIGTTOU"
        ),
    }
}
