//! Various tokens

use crate::basic_parser::{unrecoverable, Many, Parsed, Status, Token};
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
pub struct Digits(pub u32);

impl Token for Digits {
    const MAX_LEN: usize = 9;

    fn construct(s: String) -> Parsed<Self> {
        Ok(Digits(s.parse().unwrap()))
    }

    fn accept(c: char) -> bool {
        c.is_ascii_digit()
    }
}

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
        !Self::escaped(c) && !c.is_control()
    }

    const ESCAPE: char = '\\';
    fn escaped(c: char) -> bool {
        "\\,:=#".contains(c)
    }
}

impl Many for Command {}

/// An environment variable name pattern consists of alphanumeric characters as well as "_", "%" and wildcard "*"
/// (Value patterns are not supported yet)
pub struct EnvVar(pub String);

impl Token for EnvVar {
    fn construct(text: String) -> Parsed<Self> {
        Ok(EnvVar(text))
    }

    fn accept(c: char) -> bool {
        c.is_ascii_alphanumeric() || "*_%".contains(c)
    }
}

pub struct QuotedText(pub String);

impl Token for QuotedText {
    const MAX_LEN: usize = 1024;

    fn construct(s: String) -> Parsed<Self> {
        Ok(QuotedText(s))
    }

    fn accept(c: char) -> bool {
        !Self::escaped(c)
    }

    const ESCAPE: char = '\\';
    fn escaped(c: char) -> bool {
        "\\\"".contains(c) || c.is_control()
    }
}

pub struct IncludePath(pub String);

impl Token for IncludePath {
    const MAX_LEN: usize = 1024;

    fn construct(s: String) -> Parsed<Self> {
        Ok(IncludePath(s))
    }

    fn accept(c: char) -> bool {
        !c.is_control() && !Self::escaped(c)
    }

    const ESCAPE: char = '\\';
    fn escaped(c: char) -> bool {
        "\\\" ".contains(c)
    }
}

// used for Defaults where
pub struct StringParameter(pub String);

impl Token for StringParameter {
    const MAX_LEN: usize = QuotedText::MAX_LEN;

    fn construct(s: String) -> Parsed<Self> {
        Ok(StringParameter(s))
    }

    fn accept(c: char) -> bool {
        !c.is_control() && !Self::escaped(c)
    }

    const ESCAPE: char = '\\';
    fn escaped(c: char) -> bool {
        "\\\" #".contains(c)
    }
}

/// A digest specifier; note that the type of hash is implied by the length; if sudo would support
/// multiple hashes with the same hash length, this needs to be recorded explicity.
#[derive(Debug, Deref)]
pub struct Sha2(pub Box<[u8]>);

impl Token for Sha2 {
    const MAX_LEN: usize = 512 / 4;

    fn construct(s: String) -> Parsed<Self> {
        if s.len() % 2 != 0 {
            unrecoverable!("token: odd hexadecimal hash length")
        }
        let bytes: Vec<u8> = (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
            .collect::<Result<_, _>>()
            .map_err(|_| {
                Status::Fatal("should not happen: hexadecimal decoding failed".to_string())
            })?;

        Ok(Sha2(bytes.into_boxed_slice()))
    }

    fn accept(c: char) -> bool {
        ('A'..='F').contains(&c) || ('a'..='f').contains(&c) || c.is_ascii_digit()
    }
}
