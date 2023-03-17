use sudo_test::{Command, Env};

use crate::{Result, SUDOERS_USER_ALL_NOPASSWD, USERNAME};

// man sudo > Signal handling
// "As a special case, sudo will not relay signals that were sent by the command it is running."
#[test]
fn signal_sent_by_child_process_is_ignored() -> Result<()> {
    let script = include_str!("kill-sudo-parent.sh");

    let kill_sudo_parent = "/root/kill-sudo-parent.sh";
    let env = Env(SUDOERS_USER_ALL_NOPASSWD)
        .user(USERNAME)
        .file(kill_sudo_parent, script)
        .build()?;

    Command::new("sudo")
        .args(["sh", kill_sudo_parent])
        .as_user(USERNAME)
        .exec(&env)?
        .assert_success()
}
