//! Various tokens

use crate::basic_parser::{Many, Parsed, Status, Token};
use derive_more::Deref;

#[derive(Debug, Deref)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct Username(pub String);

/// A username consists of alphanumeric characters as well as "." and "-", but does not start with an underscore.
impl Token for Username {
    fn construct(text: String) -> Parsed<Self> {
        Ok(Username(text))
    }

    fn accept(c: char) -> bool {
        c.is_ascii_alphanumeric() || ".-_".contains(c)
    }

    fn accept_1st(c: char) -> bool {
        c != '_' && Self::accept(c)
    }
}

impl Many for Username {}

#[derive(Debug)]
pub struct Decimal(pub i32);

impl Token for Decimal {
    fn construct(s: String) -> Parsed<Self> {
        Ok(Decimal(s.parse().unwrap()))
    }

    fn accept(c: char) -> bool {
        c.is_ascii_digit()
    }

    fn accept_1st(c: char) -> bool {
        c.is_ascii_digit() || "+-".contains(c)
    }
}

/// A hostname consists of alphanumeric characters and ".", "-",  "_"
#[derive(Debug, Deref)]
pub struct Hostname(pub String);

impl Token for Hostname {
    fn construct(text: String) -> Parsed<Self> {
        Ok(Hostname(text))
    }

    fn accept(c: char) -> bool {
        c.is_ascii_alphanumeric() || ".-_".contains(c)
    }
}

impl Many for Hostname {}

/// A userspecifier is either a username, or a group name (TODO: user ID and group ID)
#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub enum UserSpecifier {
    User(Username),
    Group(Username),
}

impl Token for UserSpecifier {
    fn construct(text: String) -> Parsed<Self> {
        let mut chars = text.chars();
        Ok(if let Some('%') = chars.next() {
            UserSpecifier::Group(Username(chars.as_str().to_string()))
        } else {
            UserSpecifier::User(Username(text))
        })
    }

    fn accept(c: char) -> bool {
        Username::accept(c)
    }
    fn accept_1st(c: char) -> bool {
        Self::accept(c) || c == '%'
    }
}

impl Many for UserSpecifier {}

/// This enum allows items to use the ALL wildcard or be specified with aliases, or directly.
/// (Maybe this is better defined not as a Token but simply directly as an implementation of [crate::basic_parser::Parse])
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum Meta<T> {
    All,
    Only(T),
    Alias(String),
}

impl<T: Token> Token for Meta<T> {
    fn construct(s: String) -> Parsed<Self> {
        Ok(if s.chars().all(char::is_uppercase) {
            if s == "ALL" {
                Meta::All
            } else {
                Meta::Alias(s)
            }
        } else {
            Meta::Only(T::construct(s)?)
        })
    }

    const MAX_LEN: usize = T::MAX_LEN;

    fn accept(c: char) -> bool {
        T::accept(c) || c.is_uppercase()
    }
    fn accept_1st(c: char) -> bool {
        T::accept_1st(c) || c.is_uppercase()
    }

    const ESCAPE: char = T::ESCAPE;

    fn escaped(c: char) -> bool {
        T::escaped(c)
    }
}

impl<T: Many> Many for Meta<T> {
    const SEP: char = T::SEP;
    const LIMIT: usize = T::LIMIT;
}

/// An identifier that consits of only uppercase characters.
#[derive(Debug, Deref)]
pub struct Upper(pub String);

impl Token for Upper {
    fn construct(s: String) -> Parsed<Self> {
        Ok(Upper(s))
    }

    fn accept(c: char) -> bool {
        c.is_uppercase()
    }
}

/// A struct that represents valid command strings; this can contain escape sequences and are
/// limited to 1024 characters.
pub type Command = (glob::Pattern, glob::Pattern);

pub fn split_args(text: &str) -> Vec<&str> {
    text.split_whitespace().collect::<Vec<_>>()
}

impl Token for Command {
    const MAX_LEN: usize = 1024;

    fn construct(s: String) -> Parsed<Self> {
        let cvt_err = |pat: Result<_, glob::PatternError>| {
            pat.map_err(|err| Status::Fatal(format!("wildcard pattern error {}", err.msg)))
        };
        let mut cmdvec = split_args(&s);
        if cmdvec.len() == 1 {
            // if no arguments are mentioned, anything is allowed
            cmdvec.push("*");
        } else if cmdvec.len() >= 2 && cmdvec.last() == Some(&"\"\"") {
            // if the magic "" appears, no (further) arguments are allowed
            cmdvec.pop();
        }
        let cmd = cvt_err(glob::Pattern::new(cmdvec[0]))?;
        let args = cvt_err(glob::Pattern::new(&cmdvec[1..].join(" ")))?;

        Ok((cmd, args))
    }

    fn accept(c: char) -> bool {
        !Self::escaped(c)
    }

    const ESCAPE: char = '\\';
    fn escaped(c: char) -> bool {
        "\\,:=".contains(c)
    }
}

impl Many for Command {}
