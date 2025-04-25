use sudo_test::{Command, Env, BIN_TRUE};

macro_rules! assert_snapshot {
    ($($tt:tt)*) => {
        insta::with_settings!({
            prepend_module_to_snapshot => false,
            snapshot_path => "../../snapshots/sudoers/host_alias",
        }, {
            insta::assert_snapshot!($($tt)*)
        });
    };
}

#[test]
fn host_alias_works() {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "ALL SERVERS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn host_alias_can_contain_underscore_and_digits() {
    let env = Env([
        "Host_Alias UNDER_SCORE123 = ALL".to_owned(),
        format!("ALL UNDER_SCORE123 = (ALL:ALL) NOPASSWD: {BIN_TRUE}"),
    ])
    .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn host_alias_cannot_start_with_underscore() {
    let env = Env([
        "Host_Alias _FOO = ALL".to_owned(),
        format!("ALL ALL = (ALL:ALL) NOPASSWD: {BIN_TRUE}"),
        "ALL _FOO = (ALL:ALL) PASSWD: ALL".to_owned(),
    ])
    .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn host_alias_negation() {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "ALL !SERVERS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build();

    let output = Command::new("sudo").arg("true").output(&env);

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "I'm sorry root. I'm afraid I can't do that");
    }
}

#[test]
fn host_alias_double_negation() {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "ALL !!SERVERS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build();

    Command::new("sudo")
        .arg("true")
        .output(&env)
        .assert_success();
}

#[test]
fn combined_host_aliases() {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = OTHERHOSTS, !SERVERS",
        "ALL WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("foo")
    .build();

    let output = Command::new("sudo").arg("true").output(&env);
    output.assert_success();

    let second_env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = OTHERHOSTS, !SERVERS",
        "ALL WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build();

    let second_output = Command::new("sudo").arg("true").output(&second_env);
    assert!(!second_output.status().success());
    let stderr = second_output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "I'm sorry root. I'm afraid I can't do that");
    }
}

#[test]
fn unlisted_host_fails() {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = OTHERHOSTS, !SERVERS",
        "ALL WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("not_listed")
    .build();

    let output = Command::new("sudo").arg("true").output(&env);

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "I'm sorry root. I'm afraid I can't do that");
    }
}

#[test]
fn negation_not_order_sensitive() {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = !SERVERS, OTHERHOSTS",
        "ALL WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build();

    let output = Command::new("sudo").arg("true").output(&env);

    assert!(!output.status().success());

    let stderr = output.stderr();
    if sudo_test::is_original_sudo() {
        assert_snapshot!(stderr);
    } else {
        assert_contains!(stderr, "I'm sorry root. I'm afraid I can't do that");
    }
}

#[test]
fn negation_combination() {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = !SERVERS, OTHERHOSTS",
        "ALL !WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build();

    let output = Command::new("sudo").arg("true").output(&env);

    output.assert_success();
}

#[test]
fn comma_listing_works() {
    let env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = OTHERHOSTS",
        "ALL SERVERS, WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("foo")
    .build();

    let output = Command::new("sudo").arg("true").output(&env);

    output.assert_success();
    let second_env = Env([
        "Host_Alias SERVERS = main, www, mail",
        "Host_Alias OTHERHOSTS = foo, bar, baz",
        "Host_Alias WORKSTATIONS = OTHERHOSTS",
        "ALL SERVERS, WORKSTATIONS=(ALL:ALL) ALL",
    ])
    .hostname("mail")
    .build();

    let second_output = Command::new("sudo").arg("true").output(&second_env);

    second_output.assert_success();
}

#[test]
#[ignore = "gh700"]
fn keywords() {
    let hostname = "container";
    for bad_keyword in super::KEYWORDS_ALIAS_BAD {
        dbg!(bad_keyword);
        let env = Env([
            format!("Host_Alias {bad_keyword} = {hostname}"),
            format!("ALL {bad_keyword}=(ALL:ALL) ALL"),
        ])
        .hostname(hostname)
        .build();

        let output = Command::new("sudo").arg("true").output(&env);

        assert_contains!(output.stderr(), "syntax error");
        assert_eq!(*bad_keyword == "ALL", output.status().success());
    }

    for good_keyword in super::keywords_alias_good() {
        dbg!(good_keyword);
        let env = Env([
            format!("Host_Alias {good_keyword} = {hostname}"),
            format!("ALL {good_keyword}=(ALL:ALL) ALL"),
        ])
        .hostname(hostname)
        .build();

        let output = Command::new("sudo").arg("true").output(&env);

        let stderr = output.stderr();
        assert!(stderr.is_empty(), "{}", stderr);
        output.assert_success();
    }
}
