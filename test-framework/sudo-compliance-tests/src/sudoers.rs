use sudo_test::{As, EnvBuilder};

use crate::Result;

#[test]
fn cannot_sudo_with_empty_sudoers_file() -> Result<()> {
    let env = EnvBuilder::default().build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "root is not in the sudoers file");
    }

    Ok(())
}

#[test]
fn cannot_sudo_if_sudoers_file_is_world_writable() -> Result<()> {
    let env = EnvBuilder::default().sudoers_chmod("446").build()?;

    let output = env.exec(&["sudo", "true"], As::Root, None)?;
    assert_eq!(Some(1), output.status.code());

    if sudo_test::is_original_sudo() {
        assert_contains!(output.stderr, "/etc/sudoers is world writable");
    }

    Ok(())
}
