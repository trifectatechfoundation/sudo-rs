//! Various tokens

use crate::basic_parser::{Many, Token};
use derive_more::Deref;

#[derive(Debug, Deref)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct Username(pub String);

/// A username consists of alphanumeric characters as well as "." and "-", but does not start with an underscore.
impl Token for Username {
    const IDENT: fn(String) -> Self = Username;

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
    const IDENT: fn(String) -> Self = |s| Decimal(s.parse().unwrap());

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
    const IDENT: fn(String) -> Self = Hostname;

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
    const IDENT: fn(String) -> Self = |text| {
        let mut chars = text.chars();
        if let Some(c) = chars.next() {
            if c == '%' {
                return UserSpecifier::Group(Username(chars.as_str().to_string()));
            }
        }
        UserSpecifier::User(Username(text))
    };

    fn accept(c: char) -> bool {
        Username::accept(c)
    }
    fn accept_1st(c: char) -> bool {
        Self::accept(c) || c == '%'
    }
}

impl Many for UserSpecifier {}

/// This enum allows items to use the ALL wildcard as well as directly specifying items. This can
/// in the future be extended with aliases. TODO: maybe this is better defined not as a Token but
/// simply directly as an implementation of [crate::basic_parser::Parse]
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum Meta<T> {
    All,
    Only(T),
    Alias(String),
}

impl<T: Token> Token for Meta<T> {
    const IDENT: fn(String) -> Self = |s| {
        if s.chars().all(char::is_uppercase) {
            if s == "ALL" {
                Meta::All
            } else {
                Meta::Alias(s)
            }
        } else {
            Meta::Only(T::IDENT(s))
        }
    };
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
    const IDENT: fn(String) -> Self = Upper;
    fn accept(c: char) -> bool {
        c.is_uppercase()
    }
}

/// A struct that represents valid command strings; this can contain escape sequences and are
/// limited to 1024 characters.
#[derive(Debug, Deref)]
pub struct Command(pub String);

impl Token for Command {
    const MAX_LEN: usize = 1024;

    const IDENT: fn(String) -> Self = |s| Command(s.trim().to_string());

    fn accept(c: char) -> bool {
        !Self::escaped(c)
    }

    const ESCAPE: char = '\\';
    fn escaped(c: char) -> bool {
        "\\,:=".contains(c)
    }
}

impl Many for Command {}
