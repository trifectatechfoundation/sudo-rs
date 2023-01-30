use crate::basic_parser::*;
use crate::tokens::*;
use std::iter::Peekable;

/// The Sudoers file allows negating items with the exclamation mark.
#[derive(Debug)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tag {
    NoPasswd,
    Timeout(i32),
}

/// Commands with attached attributes.
#[derive(Debug)]
pub struct CommandSpec(pub Vec<Tag>, pub Spec<Command>);

/// The main AST object for one sudoer-permission line
#[derive(Debug)]
pub struct PermissionSpec {
    pub users: SpecList<UserSpecifier>,
    pub permissions: Vec<(SpecList<Hostname>, Option<RunAs>, Vec<CommandSpec>)>,
}

#[derive(Debug)]
pub struct Def<T>(pub String, pub SpecList<T>);

/// AST object for directive specifications (aliases, arguments, etc)
#[derive(Debug)]
#[allow(clippy::enum_variant_names)] // this is temporary
pub enum Directive {
    UserAlias(Def<UserSpecifier>),
    HostAlias(Def<Hostname>),
    CmndAlias(Def<Command>),
    RunasAlias(Def<UserSpecifier>),
}

/// The Sudoers file can contain permissions and directives
pub enum Sudo {
    Spec(PermissionSpec),
    Decl(Directive),
}

/// grammar:
/// ```text
/// qualified<T> = T | "!", qualified<T>
/// ```
///
/// This computes the correct negation with multiple exclamation marks in the parsing stage so we
/// are not bothered by it later.

impl<T: Parse> Parse for Qualified<T> {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        if try_syntax('!', stream).is_ok() {
            let mut neg = true;
            while try_syntax('!', stream).is_ok() {
                neg = !neg;
            }
            let ident = expect_nonterminal(stream)?;
            if neg {
                make(Qualified::Forbid(ident))
            } else {
                make(Qualified::Allow(ident))
            }
        } else {
            let ident = try_nonterminal(stream)?;
            make(Qualified::Allow(ident))
        }
    }
}

impl<T: Many> Many for Qualified<T> {
    const SEP: char = T::SEP;
    const LIMIT: usize = T::LIMIT;
}

/// grammar:
/// ```text
/// runas = "(", userlist, (":", grouplist?)?, ")"
/// ```
impl Parse for RunAs {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        try_syntax('(', stream)?;
        let users = try_nonterminal(stream).unwrap_or_default();
        let groups = maybe(try_syntax(':', stream).and_then(|_| try_nonterminal(stream)))?
            .unwrap_or_default();
        expect_syntax(')', stream)?;

        make(RunAs { users, groups })
    }
}

/// Implementing the trait Parse for `Meta<Tag>`. Wrapped in an own object to avoid
/// conflicting with a generic parse definition for [Meta].
///
/// The reason for combining a parser for these two unrelated categories is that this is one spot
/// where the sudoer grammar isn't nicely LL(1); so at the same place where "NOPASSWD" can appear,
/// we could also see "ALL".
struct MetaOrTag(Meta<Tag>);

// note: at present, "ALL" can be distinguished from a tag using a lookup of 1, since no tag starts with an "A"; but this feels like hanging onto
// the parseability by a thread (although the original sudo also has some ugly parts, like 'sha224' being an illegal user name).
// to be more general, we impl Parse for Meta<Tag> so a future tag like "AFOOBAR" can be added with no problem

impl Parse for MetaOrTag {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        use Meta::*;
        use Tag::*;
        let Upper(keyword) = try_nonterminal(stream)?;
        let result = match keyword.as_str() {
            "NOPASSWD" => NoPasswd,
            "TIMEOUT" => {
                expect_syntax('=', stream)?;
                let Decimal(t) = expect_nonterminal(stream)?;
                return make(MetaOrTag(Only(Timeout(t))));
            }
            "ALL" => return make(MetaOrTag(All)),
            alias => return make(MetaOrTag(Alias(alias.to_string()))),
        };
        expect_syntax(':', stream)?;

        make(MetaOrTag(Only(result)))
    }
}

/// grammar:
/// ```text
/// commandspec = [tags]*, command
/// ```

impl Parse for CommandSpec {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        let mut tags = Vec::new();
        while let Some(MetaOrTag(keyword)) = maybe(try_nonterminal(stream))? {
            match keyword {
                Meta::Only(tag) => tags.push(tag),
                Meta::All => return make(CommandSpec(tags, Qualified::Allow(Meta::All))),
                Meta::Alias(name) => {
                    return make(CommandSpec(tags, Qualified::Allow(Meta::Alias(name))))
                }
            }
            if tags.len() > CommandSpec::LIMIT {
                unrecoverable!("parse error: too many tags for command specifier")
            }
        }

        let cmd: Spec<Command> = expect_nonterminal(stream)?;

        make(CommandSpec(tags, cmd))
    }
}

impl Many for CommandSpec {}

/// Parsing for a tuple of hostname, runas specifier and commandspec.
/// grammar:
/// ```text
/// (host,runas,commandspec) = hostlist, "=", runas?, commandspec
/// ```

impl Parse for (SpecList<Hostname>, Option<RunAs>, Vec<CommandSpec>) {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        let hosts = try_nonterminal(stream)?;
        expect_syntax('=', stream)?;
        let runas = maybe(try_nonterminal(stream))?;
        let cmds = expect_nonterminal(stream)?;

        make((hosts, runas, cmds))
    }
}

/// A hostname, runas specifier, commandspec combination can occur multiple times in a single
/// sudoer line (seperated by ":")

impl Many for (SpecList<Hostname>, Option<RunAs>, Vec<CommandSpec>) {
    const SEP: char = ':';
}

/// grammar:
/// ```text
/// permissionspec = userlist, (host, runas, commandspec), [ ":", (host, runas, commandspec) ]*
/// ```

#[cfg(test)]
impl Parse for PermissionSpec {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        let users = try_nonterminal(stream)?;
        let permissions = expect_nonterminal(stream)?;

        make(PermissionSpec { users, permissions })
    }
}

/// grammar:
/// ```text
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
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        let users = try_nonterminal::<SpecList<_>>(stream)?;
        // element 1 always exists (parse_list fails on an empty list)
        let key = &users[0];
        if let Some(directive) = maybe(get_directive(key, stream))? {
            if users.len() != 1 {
                unrecoverable!("parse error: user name list cannot start with a directive keyword");
            }
            make(Sudo::Decl(directive))
        } else {
            let permissions = expect_nonterminal(stream)?;
            make(Sudo::Spec(PermissionSpec { users, permissions }))
        }
    }
}

fn get_directive(
    perhaps_keyword: &Spec<UserSpecifier>,
    stream: &mut Peekable<impl Iterator<Item = char>>,
) -> Parsed<Directive> {
    use crate::ast::Directive::*;
    use crate::ast::Meta::*;
    use crate::ast::Qualified::*;
    use crate::ast::UserSpecifier::*;
    let Allow(Only(User(keyword))) = perhaps_keyword else { return reject() };

    fn parse_alias<T: Token + Many>(
        ctor: fn(Def<T>) -> Directive,
        stream: &mut Peekable<impl Iterator<Item = char>>,
    ) -> Parsed<Directive> {
        let Upper(name) = expect_nonterminal(stream)?;
        expect_syntax('=', stream)?;

        make(ctor(Def(name, expect_nonterminal(stream)?)))
    }

    match keyword.as_str() {
        "User_Alias" => parse_alias(UserAlias, stream),
        "Host_Alias" => parse_alias(HostAlias, stream),
        "Cmnd_Alias" | "Cmd_Alias" => parse_alias(CmndAlias, stream),
        "Runas_Alias" => parse_alias(RunasAlias, stream),
        _ => reject(),
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
