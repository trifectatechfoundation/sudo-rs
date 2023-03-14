use pretty_assertions::assert_eq;
use sudo_test::{As, EnvBuilder};

use crate::{Result, SUDOERS_ROOT_ALL_NOPASSWD};

#[test]
fn sudo_forwards_childs_exit_code() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers(SUDOERS_ROOT_ALL_NOPASSWD)
        .build()?;

    let expected = 42;
    let output = env.exec(
        &["sudo", "sh", "-c", &format!("exit {expected}")],
        As::Root,
        None,
    )?;
    assert_eq!(Some(expected), output.status.code());

    Ok(())
}

#[test]
fn sudo_forwards_childs_stdout() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers(SUDOERS_ROOT_ALL_NOPASSWD)
        .build()?;

    let expected = "hello";
    let output = env.exec(&["sudo", "echo", expected], As::Root, None)?;
    assert_eq!(expected, output.stdout);
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn sudo_forwards_childs_stderr() -> Result<()> {
    let env = EnvBuilder::default()
        .sudoers(SUDOERS_ROOT_ALL_NOPASSWD)
        .build()?;

    let expected = "hello";
    let output = env.exec(
        &["sudo", "sh", "-c", &format!(">&2 echo {expected}")],
        As::Root,
        None,
    )?;
    assert_eq!(expected, output.stderr);
    assert!(output.stdout.is_empty());

    Ok(())
}
