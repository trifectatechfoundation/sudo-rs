use sudo_test::{Command, Env, User};

const VKILL_BYTE: u8 = b'U' & 0x1f; // 0x15
const VWERASE_BYTE: u8 = b'W' & 0x1f; // 0x17
const VERASE_BYTE: u8 = 0x7f; // DEL (default on Linux)

/// Convert a byte value to a printf octal escape string (e.g. 0x15 -> "\\025").
fn octal_escape(b: u8) -> String {
    format!("\\{:03o}", b)
}

/// Helper: run sudo inside a PTY as ferris, feeding it the given printf input.
fn run_sudo_with_input(env: &Env, input: &str) -> (bool, String) {
    let command = format!(
        // A small delay before sending input ensures sudo has time to disable
        // ICANON on the PTY
        "(sleep 0.5; printf '{input}') | script -qc 'sudo id' /dev/null 2>&1"
    );
    let output = Command::new("sh")
        .args(["-c", &command])
        .as_user("ferris")
        // Do NOT use .tty(true) — it allocates a docker PTY that interferes
        // with the PTY created by `script`.
        .output(env);
    let stdout = output.stdout_unchecked().to_string();
    // `script` always exits 0, so we detect success by checking that `id`
    // produced output containing "uid=0(root)" - which means sudo auth
    // succeeded and the command ran as root.
    let success = stdout.contains("uid=0(root)");
    (success, stdout)
}

#[test]
fn plain_password_works() {
    let env = Env("ferris ALL=(ALL:ALL) ALL")
        .user(User("ferris").password("testpass"))
        .build();
    let (success, stdout) = run_sudo_with_input(&env, "testpass\\n");
    assert!(success, "plain password failed. stdout: {stdout}");
}

#[test]
fn vkill_clears_password_field() {
    let env = Env("ferris ALL=(ALL:ALL) ALL")
        .user(User("ferris").password("testpass"))
        .build();
    let (success, stdout) = run_sudo_with_input(
        &env,
        &format!("wrong{}testpass\\n", octal_escape(VKILL_BYTE)),
    );
    assert!(
        success,
        "VKILL did not clear the password field. stdout: {stdout}"
    );
}

#[test]
fn vwerase_clears_password_field() {
    let env = Env("ferris ALL=(ALL:ALL) ALL")
        .user(User("ferris").password("testpass"))
        .build();
    let (success, stdout) = run_sudo_with_input(
        &env,
        &format!("wrong{}testpass\\n", octal_escape(VWERASE_BYTE)),
    );
    assert!(
        success,
        "VWERASE did not clear the password field. stdout: {stdout}"
    );
}

#[test]
fn verase_deletes_last_char() {
    let env = Env("ferris ALL=(ALL:ALL) ALL")
        .user(User("ferris").password("testpass"))
        .build();
    let (success, stdout) =
        run_sudo_with_input(&env, &format!("testpassX{}\\n", octal_escape(VERASE_BYTE)));
    assert!(success, "VERASE did not delete last char. stdout: {stdout}");
}

#[test]
fn verase_does_not_clear_entire_field() {
    let env = Env("ferris ALL=(ALL:ALL) ALL")
        .user(User("ferris").password("testpass"))
        .build();
    // VERASE should only delete the last char ('s'), leaving "testpas" which is wrong.
    // Feed 3 wrong passwords to exhaust all retries so sudo exits cleanly.
    let verase = octal_escape(VERASE_BYTE);
    let (success, stdout) = run_sudo_with_input(
        &env,
        &format!("testpass{verase}\\ntestpass{verase}\\ntestpass{verase}\\n"),
    );
    assert!(
        !success,
        "VERASE unexpectedly cleared entire field. stdout: {stdout}"
    );
}
