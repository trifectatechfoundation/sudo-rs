use sudo_test::{Command, Env, Output, TextFile, User};

fn launch_server(socket_path: &str, rules: String, env: &Env) -> Output {
    let cmd = format!(
        "socat - UNIX-LISTEN:{} <<< \"{}\" & disown %%",
        socket_path, rules
    );
    Command::new("/usr/bin/bash")
        .args(["-c", cmd.as_str()])
        .output(env)
}

#[test]
fn test_remote_sudoers_check() {
    let socket_path = "/secret/sudoers.socket";
    let user = "user1";
    let base_dir = "/secret";
    let include_dir = format!("{}/conf.d", base_dir);
    let include_file = format!("{}/01-conf", include_dir);

    let env = Env(format!("@socket {}", socket_path))
        .directory(sudo_test::Directory(base_dir).chmod("700"))
        .directory(sudo_test::Directory(&include_dir).chmod("700"))
        .file(&include_file, TextFile("garbage").chmod("600"))
        .user(User(user))
        .build();

    // Launch the server
    let rules = format!(
        "{} ALL=(ALL) NOPASSWD: /usr/bin/true\n@include {}\n@includedir {}\n@socket /fake/socket",
        user, include_file, include_dir
    );
    let server = launch_server(socket_path, rules, &env);
    server.assert_success();

    // Launch the client
    let output = Command::new("sudo").args(["-l", "-U", user]).output(&env);
    output.assert_success();

    // Check the results
    let stdout = output.stdout_unchecked();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);

    let words: Vec<&str> = lines[0].trim().split(' ').collect();
    assert_eq!(words.len(), 9);
    assert_eq!(words[0], "User");
    assert_eq!(words[1], user);
    assert_eq!(words[2], "may");

    assert_eq!(lines[1].trim(), "(ALL) NOPASSWD: /usr/bin/true");

    // Check the @include, @includedir and @socket directives were ignored
    let stderr = output.stderr();
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 3);

    assert_eq!(
        lines[0],
        "/secret/sudoers.socket:2:2: @include forbidden at this stage"
    );
    assert_eq!(
        lines[1],
        "/secret/sudoers.socket:3:2: @includedir forbidden at this stage"
    );
    assert_eq!(
        lines[2],
        "/secret/sudoers.socket:4:2: @socket forbidden at this stage"
    );
}

fn remote_sudoers_run(should_succeed: bool) {
    let socket_path = "/secret/sudoers.socket";
    let user = "user1";
    let base_dir = "/secret";

    let env = Env(format!("@socket {}", socket_path))
        .directory(sudo_test::Directory(base_dir).chmod("700"))
        .user(User(user))
        .build();

    // Launch the server
    let rules = format!("{} ALL=(ALL) NOPASSWD: /usr/bin/true", user);
    let server = launch_server(socket_path, rules, &env);
    server.assert_success();

    let cmd = match should_succeed {
        true => "/usr/bin/true",
        false => "/usr/bin/whoami",
    };

    // Launch the client
    let output = Command::new("sudo").arg(cmd).as_user(user).output(&env);

    match should_succeed {
        true => output.assert_success(),
        false => {
            output.assert_exit_code(1);
            let error_message = format!("sudo: I'm sorry {}. I'm afraid I can't do that", user);
            assert_eq!(output.stderr(), error_message);
        }
    };
}

#[test]
fn test_remote_sudoers_succeeds() {
    remote_sudoers_run(true);
}

#[test]
fn test_remote_sudoers_fails() {
    remote_sudoers_run(false);
}

#[test]
// This is the same test as above, except we verify that the relative socket
// which accidentally exists isn't opened
fn test_relative_remote_sudoers_fail() {
    let socket_path = "relative.socket";
    let user = "user1";
    let machine = "local";

    let env = Env(format!("@socket {}", socket_path))
        .user(User(user))
        .hostname(machine)
        .build();

    // Launch the server
    let rules = "ALL ALL=(ALL) NOPASSWD: ALL".to_string();
    let server = launch_server(&format!("/tmp/{socket_path}"), rules, &env);
    server.assert_success();

    // Launch the client
    let output = Command::new("sh")
        .args(["-c", &format!("cd /tmp; sudo -l -U {user}")])
        .output(&env);

    // Check the results
    assert_contains!(
        output.stderr(),
        format!("cannot open socket {socket_path}: path must be absolute")
    );
    assert_eq!(
        output.stdout(),
        format!("User {user} is not allowed to run sudo on {machine}.")
    );
}
