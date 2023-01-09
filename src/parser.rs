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
        if is_syntax('!', stream).is_some() {
            let mut neg = true;
            while is_syntax('!', stream).is_some() {
                neg = !neg;
            }
            let ident = expect_some(stream);
            if neg {
                Some(Qualified::Forbid(ident))
            } else {
                Some(Qualified::Allow(ident))
            }
        } else {
            let ident = is_some(stream)?;
            Some(Qualified::Allow(ident))
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
        is_syntax('(', stream)?;
        let user = is_some(stream).unwrap_or_else(|| Vec::new());
        let group = is_syntax(':', stream)
            .and_then(|_| is_some(stream))
            .unwrap_or_else(|| Vec::new());
        expect_syntax(')', stream);
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
        let Upper(keyword) = is_some(stream)?;
        let result = match keyword.as_str() {
            "NOPASSWD" => NOPASSWD,
            "TIMEOUT" => {
                expect_syntax('=', stream);
                let Decimal(t) = expect_some(stream);
                return Some(All::Only(TIMEOUT(t)));
            }
            "ALL" => return Some(All::All),
            unknown => panic!("parse error: unrecognized keyword '{}'", unknown),
        };
        expect_syntax(':', stream);
        Some(All::Only(result))
    }
}

#[derive(Debug)]
pub struct CommandSpec(pub Vec<Tag>, pub Spec<Command>);

impl Parse for CommandSpec {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let mut tags = Vec::new();
        let limit = 100;
        while let Some(keyword) = is_some(stream) {
            match keyword {
                All::Only(tag) => tags.push(tag),
                All::All => return Some(CommandSpec(tags, Qualified::Allow(All::All))),
            }
            if tags.len() > limit {
                panic!("parse error: too many tags for command specifier")
            }
        }
        let cmd = expect_some(stream);
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
        let hosts = is_some(stream)?;
        expect_syntax('=', stream);
        let runas = is_some(stream);
        let cmds = expect_some(stream);
        Some((hosts, runas, cmds))
    }
}

impl Many for (SpecList<Hostname>, Option<RunAs>, Vec<CommandSpec>) {
    const SEP: char = ':';
}

impl Parse for Sudo {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let users = is_some(stream)?;
        let permits = expect_some(stream);
        Some(Sudo {
            users: users,
            permissions: permits,
        })
    }
}
