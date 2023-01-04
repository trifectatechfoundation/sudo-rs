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
        if maybe_syntax('!', stream).is_some() {
            let mut neg = true;
            while maybe_syntax('!', stream).is_some() {
                neg = !neg;
            }
            if neg {
                Some(Qualified::Forbid(require(stream)))
            } else {
                Some(Qualified::Allow(require(stream)))
            }
        } else {
            Some(Qualified::Allow(maybe(stream)?))
        }
    }
}

impl<T: Many> Many for Qualified<T> {
    const SEP: char = T::SEP;
    const LIMIT: usize = T::LIMIT;
}

pub type Spec<T> = Qualified<All<T>>;
pub type SpecList<T> = Vec<Qualified<All<T>>>;

#[derive(Debug)]
pub struct RunAs {
    pub user: SpecList<Username>,
    pub group: SpecList<Username>,
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
pub enum Tag {
    NOPASSWD,
    TIMEOUT(i32),
}

// note: at present, "ALL" can be distinguished from a tag using a lookup of 1, since no tag starts with an "A"; but this feels like hanging onto
// the parseability by a thread (although the original sudo also has some ugly parts, like 'sha224' being an illegal user name).
// to be more general, we impl Parse for All<Tag> so a future tag like "AFOOBAR" can be added with no problem
impl Parse for All<Tag> {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        use Tag::*;
        let Upper(keyword) = maybe(stream)?;
        let result = match keyword.as_str() {
            "NOPASSWD" => NOPASSWD,
            "TIMEOUT" => {
                require_syntax('=', stream);
                let Decimal(t) = require(stream);
                return Some(All::Only(TIMEOUT(t)));
            }
            "ALL" => return Some(All::All),
            unknown => panic!("parse error: unrecognized keyword '{}'", unknown),
        };
        require_syntax(':', stream);
        Some(All::Only(result))
    }
}

#[derive(Debug)]
pub struct CommandSpec(pub Vec<Tag>, pub Spec<Command>);

impl Parse for CommandSpec {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let mut tags = Vec::new();
        let limit = 100;
        while let Some(keyword) = maybe(stream) {
            match keyword {
                All::Only(tag) => tags.push(tag),
                All::All => return Some(CommandSpec(tags, Qualified::Allow(All::All))),
            }
            if tags.len() > limit {
                panic!("parse error: too many tags for command specifier")
            }
        }
        let cmd = require(stream);
        Some(CommandSpec(tags, cmd))
    }
}

impl Many for CommandSpec {}

#[derive(Debug)]
pub struct Sudo {
    pub users: SpecList<Username>,
    pub permissions: Vec<(SpecList<Hostname>, Option<RunAs>, Vec<CommandSpec>)>,
}

impl Parse for (SpecList<Hostname>, Option<RunAs>, Vec<CommandSpec>) {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let hosts = maybe(stream)?;
        require_syntax('=', stream);
        let runas = maybe(stream);
        let cmds = require(stream);
        Some((hosts, runas, cmds))
    }
}

impl Many for (SpecList<Hostname>, Option<RunAs>, Vec<CommandSpec>) {
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
