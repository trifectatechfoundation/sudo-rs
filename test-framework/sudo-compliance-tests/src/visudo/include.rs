use sudo_test::{Command, ETC_DIR, Env, TextFile};

use crate::visudo::{CHMOD_EXEC, DEFAULT_EDITOR, EDITOR_DUMMY, LOGS_PATH};

#[test]
#[ignore = "gh657"]
fn prompt() {
    let env = Env("@include sudoers2")
        .file(format!("{ETC_DIR}/sudoers2"), "")
        .file(DEFAULT_EDITOR, TextFile(EDITOR_DUMMY).chmod(CHMOD_EXEC))
        .build();

    let output = Command::new("visudo").output(&env);

    assert_contains!(
        output.stdout(),
        format!("press return to edit {ETC_DIR}/sudoers2:")
    );
}

#[test]
#[ignore = "gh657"]
fn calls_editor_on_included_files() {
    let env = Env("@include sudoers2")
        .file(format!("{ETC_DIR}/sudoers2"), "")
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                "#!/bin/sh
echo $@ >> {LOGS_PATH}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("visudo")
        .stdin("\n")
        .output(&env)
        .assert_success();
    let logs = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    let lines = logs.lines().collect::<Vec<_>>();

    assert_eq!(2, lines.len());
}

#[test]
#[ignore = "gh657"]
fn closing_stdin_is_understood_as_yes_to_all() {
    let env = Env("@include sudoers2
@include sudoers3")
    .file(format!("{ETC_DIR}/sudoers2"), "")
    .file(format!("{ETC_DIR}/sudoers3"), "")
    .file(
        DEFAULT_EDITOR,
        TextFile(format!(
            "#!/bin/sh
echo $@ >> {LOGS_PATH}"
        ))
        .chmod(CHMOD_EXEC),
    )
    .build();

    Command::new("visudo").output(&env).assert_success();
    let logs = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    let lines = logs.lines().collect::<Vec<_>>();

    assert_eq!(3, lines.len());
}

#[test]
#[ignore = "gh657"]
fn edit_order_follows_include_order() {
    let env = Env("# 1
@include sudoers2
@include sudoers4")
    .file(
        format!("{ETC_DIR}/sudoers2"),
        "# 2
@include sudoers3",
    )
    .file(format!("{ETC_DIR}/sudoers3"), "# 3")
    .file(format!("{ETC_DIR}/sudoers4"), "# 4")
    .file(
        DEFAULT_EDITOR,
        TextFile(format!(
            "#!/bin/sh
cat $2 >> {LOGS_PATH}"
        ))
        .chmod(CHMOD_EXEC),
    )
    .build();

    Command::new("visudo").output(&env).assert_success();
    let logs = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    let comments = logs
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect::<Vec<_>>();

    assert_eq!(["# 1", "# 2", "# 3", "# 4"], &*comments);
}

#[test]
#[ignore = "gh657"]
fn include_cycle_does_not_edit_the_same_files_many_times() {
    let env = Env("# 1
@include sudoers2")
    .file(
        format!("{ETC_DIR}/sudoers2"),
        "# 2
@include sudoers",
    )
    .file(
        DEFAULT_EDITOR,
        TextFile(format!(
            "#!/bin/sh
cat $2 >> {LOGS_PATH}"
        ))
        .chmod(CHMOD_EXEC),
    )
    .build();

    let output = Command::new("visudo").output(&env);
    output.assert_success();

    // NOTE ogvisudo reports this twice
    assert_contains!(
        output.stderr(),
        format!("{ETC_DIR}/sudoers2: too many levels of includes")
    );

    let logs = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    let comments = logs
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect::<Vec<_>>();

    assert_eq!(["# 1", "# 2"], &*comments);
}

#[test]
#[ignore = "gh657"]
fn does_edit_at_include_added_in_last_edit() {
    let env = Env("# 1")
        .file(format!("{ETC_DIR}/sudoers2"), "# 2")
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                "#!/bin/sh
cp $2 /tmp/scratchpad
[ -f {LOGS_PATH} ] || echo '@include sudoers2' >> $2
cat /tmp/scratchpad >> {LOGS_PATH}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build();

    Command::new("visudo").output(&env).assert_success();

    let logs = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    let comments = logs
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect::<Vec<_>>();

    assert_eq!(["# 1", "# 2"], &*comments);
}

#[test]
#[ignore = "gh657"]
fn does_edit_at_include_removed_in_last_edit() {
    let env = Env("# 1
@include sudoers2")
    .file(format!("{ETC_DIR}/sudoers2"), "# 2")
    .file(
        DEFAULT_EDITOR,
        TextFile(format!(
            "#!/bin/sh
cp $2 /tmp/scratchpad
[ -f {LOGS_PATH} ] || echo '' > $2
cat /tmp/scratchpad >> {LOGS_PATH}"
        ))
        .chmod(CHMOD_EXEC),
    )
    .build();

    Command::new("visudo").output(&env).assert_success();

    let logs = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    let comments = logs
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect::<Vec<_>>();

    assert_eq!(["# 1", "# 2"], &*comments);
}

#[test]
#[ignore = "gh657"]
fn edits_existing_at_includes_first_then_newly_added_at_includes() {
    let env = Env("# 1
@include sudoers2")
    .file(format!("{ETC_DIR}/sudoers2"), "# 2")
    .file(format!("{ETC_DIR}/sudoers3"), "# 3")
    .file(
        DEFAULT_EDITOR,
        TextFile(format!(
            "#!/bin/sh
cp $2 /tmp/scratchpad
[ -f {LOGS_PATH} ] || echo '@include sudoers3' > $2
cat /tmp/scratchpad >> {LOGS_PATH}"
        ))
        .chmod(CHMOD_EXEC),
    )
    .build();

    Command::new("visudo").output(&env).assert_success();

    let logs = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    let comments = logs
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect::<Vec<_>>();

    assert_eq!(["# 1", "# 2", "# 3"], &*comments);
}

#[test]
fn does_not_edit_files_in_includedir_directories() {
    let env = Env(format!(
        "# 1
    @includedir {ETC_DIR}/sudoers.d"
    ))
    .file(format!("{ETC_DIR}/sudoers.d/a"), "# 2")
    .file(
        DEFAULT_EDITOR,
        TextFile(format!(
            "#!/bin/sh
cat $2 >> {LOGS_PATH}"
        ))
        .chmod(CHMOD_EXEC),
    )
    .build();

    Command::new("visudo").output(&env).assert_success();

    let logs = Command::new("cat").arg(LOGS_PATH).output(&env).stdout();

    let comments = logs
        .lines()
        .filter(|line| line.starts_with('#'))
        .collect::<Vec<_>>();

    assert_eq!(["# 1"], &*comments);
}
