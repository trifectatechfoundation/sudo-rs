use crate::basic_parser::*;
use crate::tokens::*;
use std::iter::Peekable;

#[derive(Debug)]
pub enum Qualified<T> {
    Allow(T),
    Forbid(T),
}

impl<T: Token> Parse for Qualified<T> {
    fn parse(iter: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let mut neg = false;
        while maybe_syntax('!', iter).is_some() {
            neg = !neg
        }
        let elem = T::parse(iter)?;
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
    fn parse(iter: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        maybe_syntax('(', iter)?;
        let user = maybe(iter).unwrap_or_else(|| Vec::new());
        let group = maybe_syntax(':', iter)
            .and_then(|_| maybe(iter))
            .unwrap_or_else(|| Vec::new());
        require_syntax(')', iter);
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
    fn parse(iter: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let hosts = maybe(iter)?;
        require_syntax('=', iter);
        let runas = maybe(iter);
        let cmds = require(iter);
        Some((hosts, runas, cmds))
    }
}

impl Many for (Spec<Hostname>, Option<RunAs>, Spec<Command>) {
    const SEP: char = ':';
}

impl Parse for Sudo {
    fn parse(iter: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let users = maybe(iter)?;
        let permits = require(iter);
        Some(Sudo {
            users: users,
            permissions: permits,
        })
    }
}
