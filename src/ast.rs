use crate::basic_parser::*;
use crate::tokens::*;
use std::iter::Peekable;

/// The Sudoers file allows negating items with the exclamation mark.
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum Qualified<T> {
    Allow(T),
    Forbid(T),
}

/// Type aliases; many items can be replaced by ALL, aliases, and negated.
pub type Spec<T> = Qualified<Meta<T>>;
pub type SpecList<T> = Vec<Spec<T>>;

/// The RunAs specification consists of a (possibly empty) list of userspecifiers, followed by a (possibly empty) list of groups.
#[derive(Debug, Default)]
pub struct RunAs {
    pub users: SpecList<UserSpecifier>,
    pub groups: SpecList<Username>,
}

/// Commands in /etc/sudoers can have attributes attached to them, such as NOPASSWD, NOEXEC, ...
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tag {
    NOPASSWD,
    TIMEOUT(i32),
}

/// Commands with attached attributes.
#[derive(Debug, Clone)]
pub struct CommandSpec(pub Vec<Tag>, pub Spec<Command>);

/// The main AST object for one sudoer-permission line
#[derive(Debug)]
pub struct PermissionSpec {
    pub users: SpecList<UserSpecifier>,
    pub permissions: Vec<(SpecList<Hostname>, Option<RunAs>, Vec<CommandSpec>)>,
}

// AST object for directive specifications (aliases, arguments, etc)
#[derive(Debug)]
pub struct Def<T>(pub String, pub SpecList<T>);

#[derive(Debug)]
pub enum Directive {
    UserAlias(Def<UserSpecifier>),
}

// The main AST object for sudo directives (including alias definitions)
/// The Sudoers file can contain permissions and directives
pub enum Sudo {
    Spec(PermissionSpec),
    Decl(Directive),
}

/// grammar:
/// ```
/// qualified<T> = T | "!", qualified<T>
/// ```
///
/// This computes the correct negation with multiple exclamation marks in the parsing stage so we
/// are not bothered by it later.

impl<T: Parse> Parse for Qualified<T> {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        if try_syntax('!', stream).is_some() {
            let mut neg = true;
            while try_syntax('!', stream).is_some() {
                neg = !neg;
            }
            let ident = expect_nonterminal(stream);
            if neg {
                Some(Qualified::Forbid(ident))
            } else {
                Some(Qualified::Allow(ident))
            }
        } else {
            let ident = try_nonterminal(stream)?;
            Some(Qualified::Allow(ident))
        }
    }
}

impl<T: Many> Many for Qualified<T> {
    const SEP: char = T::SEP;
    const LIMIT: usize = T::LIMIT;
}

/// grammar:
/// ```
/// runas = "(", userlist, (":", grouplist?)?, ")"
/// ```

impl Parse for RunAs {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        try_syntax('(', stream)?;
        let users = try_nonterminal(stream).unwrap_or_default();
        let groups = try_syntax(':', stream)
            .and_then(|_| try_nonterminal(stream))
            .unwrap_or_default();
        expect_syntax(')', stream);
        Some(RunAs { users, groups })
    }
}

/// Implementing the trait `Meta<Tag>`. Note that [Tag] does not implement [crate::basic_parser::Token]
/// so this does not conflict with the generic definition for [Meta].
///
/// The reason for combining a parser for these two unrelated categories is that this is one spot
/// where the sudoer grammar isn't nicely LL(1); so at the same place where "NOPASSWD" can appear,
/// we could also see "ALL".

// note: at present, "ALL" can be distinguished from a tag using a lookup of 1, since no tag starts with an "A"; but this feels like hanging onto
// the parseability by a thread (although the original sudo also has some ugly parts, like 'sha224' being an illegal user name).
// to be more general, we impl Parse for Meta<Tag> so a future tag like "AFOOBAR" can be added with no problem
impl Parse for Meta<Tag> {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        use Tag::*;
        let Upper(keyword) = try_nonterminal(stream)?;
        let result = match keyword.as_str() {
            "NOPASSWD" => NOPASSWD,
            "TIMEOUT" => {
                expect_syntax('=', stream);
                let Decimal(t) = expect_nonterminal(stream);
                return Some(Meta::Only(TIMEOUT(t)));
            }
            "ALL" => return Some(Meta::All),
            unknown => panic!("parse error: unrecognized keyword '{unknown}'"),
        };
        expect_syntax(':', stream);
        Some(Meta::Only(result))
    }
}

/// grammar:
/// ```
/// commandspec = [tags]*, command
/// ```

impl Parse for CommandSpec {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let mut tags = Vec::new();
        let limit = 100;
        while let Some(keyword) = try_nonterminal(stream) {
            match keyword {
                Meta::Only(tag) => tags.push(tag),
                Meta::All => return Some(CommandSpec(tags, Qualified::Allow(Meta::All))),
                _ => todo!(),
            }
            if tags.len() > limit {
                panic!("parse error: too many tags for command specifier")
            }
        }
        let cmd = expect_nonterminal(stream);
        Some(CommandSpec(tags, cmd))
    }
}

impl Many for CommandSpec {}

/// Parsing for a tuple of hostname, runas specifier and commandspec.
/// grammar:
/// ```
/// (host,runas,commandspec) = hostlist, "=", runas?, commandspec
/// ```

impl Parse for (SpecList<Hostname>, Option<RunAs>, Vec<CommandSpec>) {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let hosts = try_nonterminal(stream)?;
        expect_syntax('=', stream);
        let runas = try_nonterminal(stream);
        let cmds = expect_nonterminal(stream);
        Some((hosts, runas, cmds))
    }
}

/// A hostname, runas specifier, commandspec combination can occur multiple times in a single
/// sudoer line (seperated by ":")

impl Many for (SpecList<Hostname>, Option<RunAs>, Vec<CommandSpec>) {
    const SEP: char = ':';
}

/// grammar:
/// ```
/// permissionspec = userlist, (host, runas, commandspec), [ ":", (host, runas, commandspec) ]*
/// ```

#[allow(dead_code)]
impl Parse for PermissionSpec {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let users = try_nonterminal(stream)?;
        let permissions = expect_nonterminal(stream);
        Some(PermissionSpec { users, permissions })
    }
}

/// grammar:
/// ```
/// sudo = permissionspec
///      | Keyword identifier = identifier_list
/// ```
/// There is a syntactical ambiguity in the sudoer Directive and Permission specifications, so we
/// have to parse them 'together' and do a delayed decision on which category we are in.

impl Parse for Sudo {
    // note: original sudo would reject:
    //   "User_Alias, user machine = command"
    // but accept:
    //   "user, User_Alias machine = command"; this does the same
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let users = try_nonterminal::<SpecList<_>>(stream)?;
        // element 1 always exists (parse_list fails on an empty list)
        let key = &users[0];
        if let Some(directive) = get_directive(key, stream) {
            if users.len() != 1 {
                panic!("parse error: user name list cannot start with a directive keyword");
            }
            Some(Sudo::Decl(directive))
        } else {
            let permissions = expect_nonterminal(stream);
            Some(Sudo::Spec(PermissionSpec { users, permissions }))
        }
    }
}

fn get_directive(
    perhaps_keyword: &Spec<UserSpecifier>,
    stream: &mut Peekable<impl Iterator<Item = char>>,
) -> Option<Directive> {
    use crate::ast::Directive::*;
    use crate::ast::Meta::*;
    use crate::ast::Qualified::*;
    use crate::ast::UserSpecifier::*;
    let Allow(Only(User(keyword))) = perhaps_keyword else { return None };
    match keyword.as_str() {
        "User_Alias" => {
            let name = expect_nonterminal::<Upper>(stream);
            expect_syntax('=', stream);
            Some(UserAlias(Def(name.to_string(), expect_nonterminal(stream))))
        }
        _ => None,
    }
}

/// A bit of the hack to make semantic analysis easier: a CommandSpec has attributes, but most
/// other elements that occur in a [crate::ast::Qualified] wrapper do not.
/// The [Tagged] trait allows getting these tags (defaulting to `()`, i.e. no attributes)

pub trait Tagged<U> {
    type Flags;
    fn into(&self) -> &Spec<U>;
    fn to_info(&self) -> &Self::Flags;
}

pub const NO_TAG: &() = &();

/// Default implementation

impl<T> Tagged<T> for Spec<T> {
    type Flags = ();
    fn into(&self) -> &Spec<T> {
        self
    }
    fn to_info(&self) -> &() {
        NO_TAG
    }
}

/// Special implementation for [CommandSpec]

impl Tagged<Command> for CommandSpec {
    type Flags = Vec<Tag>;
    fn into(&self) -> &Spec<Command> {
        &self.1
    }
    fn to_info(&self) -> &Self::Flags {
        &self.0
    }
}
