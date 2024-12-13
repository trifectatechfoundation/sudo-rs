#![forbid(unsafe_code)]
// FUTURE IDEA: use a representation that allows for more Rust-type structure rather than passing
// strings around; some settings in sudoers file are more naturally represented like that, such as
// "verifypw" and "logfile"
pub enum SudoDefault {
    Flag(bool),
    Integer(OptTuple<i64>, fn(&str) -> Option<i64>),
    Text(OptTuple<Option<&'static str>>),
    List(&'static [&'static str]),
    Enum(OptTuple<StrEnum<'static>>),
}

pub type SettingsModifier = Box<dyn FnOnce(&mut Settings)>;

pub enum ListMode {
    Set,
    Add,
    Del,
}

pub enum SettingKind {
    Flag(SettingsModifier),
    Integer(fn(&str) -> Option<SettingsModifier>),
    Text(fn(&str) -> Option<SettingsModifier>),
    List(fn(ListMode, Vec<String>) -> Option<SettingsModifier>),
}

#[derive(Debug)]
pub struct OptTuple<T> {
    pub default: T,
    pub negated: Option<T>,
}

mod strenum;
pub use strenum::StrEnum;

mod settings_dsl;
use settings_dsl::*;

defaults! {
    always_query_group_plugin = false
    always_set_home           = false
    env_reset                 = true
    mail_badpass              = true
    match_group_by_gid        = false
    use_pty                   = true
    visiblepw                 = false
    env_editor                = true

    passwd_tries              = 3 [0..=1000]

    secure_path               = None (!= None)

    verifypw                  = all (!= never) [all, always, any, never]

    timestamp_timeout         = (15*60) (!= 0) {fractional_minutes}

    env_keep                  = ["COLORS", "DISPLAY", "HOSTNAME", "KRB5CCNAME", "LS_COLORS", "PATH",
                                 "PS1", "PS2", "XAUTHORITY", "XAUTHORIZATION", "XDG_CURRENT_DESKTOP"]

    env_check                 = ["COLORTERM", "LANG", "LANGUAGE", "LC_*", "LINGUAS", "TERM", "TZ"]

    env_delete                = ["IFS", "CDPATH", "LOCALDOMAIN", "RES_OPTIONS", "HOSTALIASES",
                                "NLSPATH", "PATH_LOCALE", "LD_*", "_RLD*", "TERMINFO", "TERMINFO_DIRS",
                                "TERMPATH", "TERMCAP", "ENV", "BASH_ENV", "PS4", "GLOBIGNORE",
                                "BASHOPTS", "SHELLOPTS", "JAVA_TOOL_OPTIONS", "PERLIO_DEBUG",
                                "PERLLIB", "PERL5LIB", "PERL5OPT", "PERL5DB", "FPATH", "NULLCMD",
                                "READNULLCMD", "ZDOTDIR", "TMPPREFIX", "PYTHONHOME", "PYTHONPATH",
                                "PYTHONINSPECT", "PYTHONUSERBASE", "RUBYLIB", "RUBYOPT", "*=()*"]
}

old_defaults! {
    always_query_group_plugin = false
    always_set_home           = false
    env_reset                 = true
    mail_badpass              = true
    match_group_by_gid        = false
    use_pty                   = true
    visiblepw                 = false
    env_editor                = true

    passwd_tries              = 3 [0..=1000]

    secure_path               = None (!= None)
    verifypw                  = "all" (!= "never") [all, always, any, never]

    timestamp_timeout         = (15*60) (!= 0) {fractional_minutes}

    env_keep                  = ["COLORS", "DISPLAY", "HOSTNAME", "KRB5CCNAME", "LS_COLORS", "PATH",
                                 "PS1", "PS2", "XAUTHORITY", "XAUTHORIZATION", "XDG_CURRENT_DESKTOP"]

    env_check                 = ["COLORTERM", "LANG", "LANGUAGE", "LC_*", "LINGUAS", "TERM", "TZ"]

    env_delete                = ["IFS", "CDPATH", "LOCALDOMAIN", "RES_OPTIONS", "HOSTALIASES",
                                "NLSPATH", "PATH_LOCALE", "LD_*", "_RLD*", "TERMINFO", "TERMINFO_DIRS",
                                "TERMPATH", "TERMCAP", "ENV", "BASH_ENV", "PS4", "GLOBIGNORE",
                                "BASHOPTS", "SHELLOPTS", "JAVA_TOOL_OPTIONS", "PERLIO_DEBUG",
                                "PERLLIB", "PERL5LIB", "PERL5OPT", "PERL5DB", "FPATH", "NULLCMD",
                                "READNULLCMD", "ZDOTDIR", "TMPPREFIX", "PYTHONHOME", "PYTHONPATH",
                                "PYTHONINSPECT", "PYTHONUSERBASE", "RUBYLIB", "RUBYOPT", "*=()*"]
}

/// A custom parser to parse seconds as fractional "minutes", the format used by
/// passwd_timeout and timestamp_timeout.
fn fractional_minutes(input: &str) -> Option<i64> {
    if input.contains('.') {
        Some((input.parse::<f64>().ok()? * 60.0).floor() as i64)
    } else {
        Some(input.parse::<i64>().ok()? * 60)
    }
}

#[cfg(test)]
mod old_test {
    use super::*;

    #[test]
    fn check() {
        macro_rules! test {
            ($name:ident => $value:pat) => {
                let Some(foo @ $value) = sudo_default(stringify!($name)) else {
                    unreachable!()
                };
                if let SudoDefault::Enum(OptTuple { default, negated }) = foo {
                    assert!(default
                        .possible_values
                        .iter()
                        .any(|x| *x as *const str == default.get()));
                    negated.map(|neg| assert!(neg.possible_values.contains(&neg.get())));
                }
            };
        }
        assert!(sudo_default("bla").is_none());

        use SudoDefault::*;

        test! { always_query_group_plugin => Flag(false) };
        test! { always_set_home => Flag(false) };
        test! { env_reset => Flag(true) };
        test! { mail_badpass => Flag(true) };
        test! { match_group_by_gid => Flag(false) };
        test! { use_pty => Flag(true) };
        test! { visiblepw => Flag(false) };
        test! { env_editor => Flag(true) };
        test! { passwd_tries => Integer(OptTuple { default: 3, negated: None }, _) };
        test! { secure_path => Text(OptTuple { default: None, negated: Some(None) }) };
        test! { env_keep => List(_) };
        test! { env_check => List(["COLORTERM", "LANG", "LANGUAGE", "LC_*", "LINGUAS", "TERM", "TZ"]) };
        test! { env_delete => List(_) };
        test! { verifypw => Enum(OptTuple { default: StrEnum { value: "all", possible_values: [_, "always", "any", _] }, negated: Some(StrEnum { value: "never", .. }) }) };

        let myenum = StrEnum::new("hello", &["hello", "goodbye"]).unwrap();
        assert!(&myenum as &str == "hello");
    }
}
#[cfg(test)]
mod test {
    use super::*;

    #[allow(clippy::bool_assert_comparison)]
    #[test]
    fn check() {
        let mut def = Settings::default();
        assert_eq! { def.always_query_group_plugin, false };
        assert_eq! { def.always_set_home, false };
        assert_eq! { def.env_reset, true };
        assert_eq! { def.mail_badpass, true };
        assert_eq! { def.match_group_by_gid, false };
        assert_eq! { def.use_pty, true };
        assert_eq! { def.visiblepw, false };
        assert_eq! { def.env_editor, true };
        assert_eq! { def.passwd_tries, 3 };
        assert_eq! { def.secure_path, None };
        assert_eq! { def.env_check, ["COLORTERM", "LANG", "LANGUAGE", "LC_*", "LINGUAS", "TERM", "TZ"].iter().map(|s| s.to_string()).collect() };
        assert_eq! { def.verifypw, enums::verifypw::all };

        negate("env_check").unwrap()(&mut def);
        negate("env_reset").unwrap()(&mut def);
        negate("secure_path").unwrap()(&mut def);
        negate("verifypw").unwrap()(&mut def);
        assert_eq! { def.always_query_group_plugin, false };
        assert_eq! { def.always_set_home, false };
        assert_eq! { def.env_reset, false };
        assert_eq! { def.mail_badpass, true };
        assert_eq! { def.match_group_by_gid, false };
        assert_eq! { def.use_pty, true };
        assert_eq! { def.visiblepw, false };
        assert_eq! { def.env_editor, true };
        assert_eq! { def.passwd_tries, 3 };
        assert_eq! { def.secure_path, None };
        assert! { def.env_check.is_empty() };
        assert_eq! { def.verifypw, enums::verifypw::never };

        let SettingKind::Flag(f) = set("env_reset").unwrap() else {
            panic!()
        };
        f(&mut def);
        let SettingKind::Text(f) = set("secure_path").unwrap() else {
            panic!()
        };
        f("/bin").unwrap()(&mut def);
        let SettingKind::Integer(f) = set("passwd_tries").unwrap() else {
            panic!()
        };
        f("5").unwrap()(&mut def);
        let SettingKind::Text(f) = set("verifypw").unwrap() else {
            panic!()
        };
        f("any").unwrap()(&mut def);
        assert_eq! { def.always_query_group_plugin, false };
        assert_eq! { def.always_set_home, false };
        assert_eq! { def.env_reset, true };
        assert_eq! { def.mail_badpass, true };
        assert_eq! { def.match_group_by_gid, false };
        assert_eq! { def.use_pty, true };
        assert_eq! { def.visiblepw, false };
        assert_eq! { def.env_editor, true };
        assert_eq! { def.passwd_tries, 5 };
        assert_eq! { def.secure_path, Some("/bin".into()) };
        assert! { def.env_check.is_empty() };
        assert_eq! { def.verifypw, enums::verifypw::any };

        assert!(set("notanoption").is_none());
        assert!(f("notanoption").is_none());
    }
}
