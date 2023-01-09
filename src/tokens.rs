use crate::basic_parser::{Many, Token};

#[derive(Debug, PartialEq, Eq)]
pub struct Username(pub String);

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

#[derive(Debug)]
pub struct Hostname(pub String);

impl Token for Hostname {
    const IDENT: fn(String) -> Self = Hostname;

    fn accept(c: char) -> bool {
        c.is_ascii_alphanumeric() || ".-_".contains(c)
    }
}

impl Many for Hostname {}

#[derive(Debug)]
pub enum All<T> {
    All,
    Only(T),
}

impl<T: Token> Token for All<T> {
    const IDENT: fn(String) -> Self = |s| {
        if s == "ALL" {
            All::All
        } else {
            All::Only(T::IDENT(s))
        }
    };
    const MAX_LEN: usize = T::MAX_LEN;
    fn accept(c: char) -> bool {
        T::accept(c) || c == 'L'
    }
    fn accept_1st(c: char) -> bool {
        T::accept_1st(c) || c == 'A'
    }

    const ESCAPE: char = T::ESCAPE;
    fn escaped(c: char) -> bool {
        T::escaped(c)
    }
}

impl<T: Many> Many for All<T> {
    const SEP: char = T::SEP;
    const LIMIT: usize = T::LIMIT;
}

#[derive(Debug)]
pub struct Upper(pub String);

impl Token for Upper {
    const IDENT: fn(String) -> Self = Upper;
    fn accept(c: char) -> bool {
        c.is_uppercase()
    }
}

#[derive(Debug)]
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
