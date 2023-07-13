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
mod test {
    use super::*;

    #[test]
    fn check() {
        macro_rules! test {
            ($name:ident => $value:pat) => {
                let Some(foo@$value) = sudo_default(stringify!($name)) else { unreachable!() };
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
