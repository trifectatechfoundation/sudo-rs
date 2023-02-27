#[derive(Debug)]
pub enum SudoDefault {
    Flag(bool),
    Integer(OptTuple<usize>),
    Text(OptTuple<&'static str>),
    List(&'static [&'static str]),
}

type OptTuple<T> = (T, Option<T>);

mod settings_dsl;
use settings_dsl::*;

defaults! {
    env_reset       = true

    passwd_tries    = 3
    umask           = 0o22 (!= 0o777)

    editor          = "/usr/bin/editor"
    verifypw        = "all" (!= "never")

    env_keep        = ["*=()*", "XDG_CURRENT_DESKTOP", "XAUTHORIZATION", "XAUTHORITY", "PS2", "PS1", "PATH", "LS_COLORS", "KRB5CCNAME", "HOSTNAME", "DPKG_COLORS", "DISPLAY", "COLORS"]
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
        test! { passwd_tries => Integer((3,None)) };
        test! { editor       => Text((_, None))};
        test! { env_keep     => List(_)};
        test! { umask        => Integer((18, Some(511))) };
        test! { verifypw     => Text(("all", Some("never"))) };
    }
}
