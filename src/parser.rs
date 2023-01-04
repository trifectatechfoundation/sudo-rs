use crate::basic_parser::*;
use crate::tokens::*;
use std::iter::Peekable;

#[derive(Debug)]
pub enum Qualified<T> {
    Allow(T),
    Forbid(T),
}

impl<T: Token> Parse for Qualified<T> {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let mut neg = false;
        while maybe_syntax('!', stream).is_some() {
            neg = !neg
        }
        let elem = T::parse(stream)?;
        Some(if !neg {
            Qualified::Allow(elem)
        } else {
            Qualified::Forbid(elem)
        })
    }
}

impl<T: Many> Many for Qualified<T> {
    const SEP: char = T::SEP;
    const LIMIT: usize = T::LIMIT;
}

pub type Spec<T> = Vec<Qualified<All<T>>>;

#[derive(Debug)]
#[allow(dead_code)]
pub struct RunAs {
    user: Spec<Username>,
    group: Spec<Username>,
}

impl Parse for RunAs {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        maybe_syntax('(', stream)?;
        let user = maybe(stream).unwrap_or_else(|| Vec::new());
        let group = maybe_syntax(':', stream)
            .and_then(|_| maybe(stream))
            .unwrap_or_else(|| Vec::new());
        require_syntax(')', stream);
        Some(RunAs {
            user: user,
            group: group,
        })
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct Sudo {
    users: Spec<Username>,
    permissions: Vec<(Spec<Hostname>, Option<RunAs>, Spec<Command>)>,
}

impl Parse for (Spec<Hostname>, Option<RunAs>, Spec<Command>) {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let hosts = maybe(stream)?;
        require_syntax('=', stream);
        let runas = maybe(stream);
        let cmds = require(stream);
        Some((hosts, runas, cmds))
    }
}

impl Many for (Spec<Hostname>, Option<RunAs>, Spec<Command>) {
    const SEP: char = ':';
}

impl Parse for Sudo {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let users = maybe(stream)?;
        let permits = require(stream);
        Some(Sudo {
            users: users,
            permissions: permits,
        })
    }
}
