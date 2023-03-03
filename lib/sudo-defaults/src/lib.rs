// TODO: add "allowed:" restrictions on string parameters that are enum-like; and maybe also on
// integers that have a particular range restriction
//
// FUTURE IDEA: use a representation that allows for more Rust-type structure rather than passing
// strings around; some settings in sudoers file are more naturally represented like that, such as
// "verifypw" and "logfile"
#[derive(Debug)]
pub enum SudoDefault {
    Flag(bool),
    Integer(OptTuple<i128>),
    Text(OptTuple<&'static str>),
    List(&'static [&'static str]),
}

#[derive(Debug)]
pub struct OptTuple<T> {
    pub default: T,
    pub negated: Option<T>,
}

mod settings_dsl;
use settings_dsl::*;

defaults! {
    env_reset       = true
    mail_badpass    = true
    use_pty         = false

    passwd_tries    = 3
    umask           = 0o22 (!= 0o777)

    editor          = "/usr/bin/editor"

    secure_path     = "" (!= "")
    verifypw        = "all" (!= "never")


    env_keep        = ["XDG_CURRENT_DESKTOP", "XAUTHORIZATION", "XAUTHORITY", "PS2", "PS1", "PATH", "LS_COLORS", "KRB5CCNAME", "HOSTNAME", "DPKG_COLORS", "DISPLAY", "COLORS"]

    env_delete      = ["*=()*", "RUBYOPT", "RUBYLIB", "PYTHONUSERBASE", "PYTHONINSPECT", "PYTHONPATH", "PYTHONHOME", "TMPPREFIX", "ZDOTDIR", "READNULLCMD", "NULLCMD", "FPATH",
                       "PERL5DB", "PERL5OPT", "PERL5LIB", "PERLLIB", "PERLIO_DEBUG", "JAVA_TOOL_OPTIONS", "SHELLOPTS", "BASHOPTS", "GLOBIGNORE", "PS4", "BASH_ENV", "ENV",
                       "TERMCAP", "TERMPATH", "TERMINFO_DIRS", "TERMINFO", "_RLD*", "LD_*", "PATH_LOCALE", "NLSPATH", "HOSTALIASES", "RES_OPTIONS", "LOCALDOMAIN", "CDPATH", "IFS"]
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn check() {
        macro_rules! test {
            ($name:ident => $value:pat) => {
                let Some($value) = sudo_default(stringify!($name)) else { unreachable!() };
            };
        }
        assert!(sudo_default("bla").is_none());

        use SudoDefault::*;
        test! { env_reset    => Flag(true) };
        test! { passwd_tries => Integer(OptTuple { default: 3, negated: None }) };
        test! { editor       => Text(_) };
        test! { env_keep     => List(_) };
        test! { umask        => Integer(OptTuple { default: 18, negated: Some(511) }) };
        test! { verifypw     => Text(OptTuple { default: "all", negated: Some("never") }) };
    }
}
