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

/// A userspecifier is either a username, or a (non-unix) group name, or netgroup
#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub enum UserSpecifier {
    User(Identifier),
    Group(Identifier),
    NonunixGroup(Identifier),
}

pub use crate::tokens::Identifier;

/// The RunAs specification consists of a (possibly empty) list of userspecifiers, followed by a (possibly empty) list of groups.
#[derive(Debug, Default)]
pub struct RunAs {
    pub users: SpecList<UserSpecifier>,
    pub groups: SpecList<Identifier>,
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
pub enum Directive {
    UserAlias(Def<UserSpecifier>),
    HostAlias(Def<Hostname>),
    CmndAlias(Def<Command>),
    RunasAlias(Def<UserSpecifier>),
    Defaults(String, DefaultValue),
}

#[derive(Debug)]
//TODO: integer values and "boolean context strings/lists/integers"
pub enum DefaultValue {
    Flag(bool),
    Text(String),

    // encoding: -1 = subtract, 0 = set, +1 = add
    List(Mode, Vec<String>),
}

#[derive(Debug)]
pub enum Mode {
    Add,
    Set,
    Del,
}

/// The Sudoers file can contain permissions and directives
#[derive(Debug)]
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

/// Since UserSpecifier is not a token, implement the parser for Meta<UserSpecifier>
impl Parse for Meta<UserSpecifier> {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        use UserSpecifier::*;
        if let Some(meta) = maybe(try_nonterminal::<Meta<Username>>(stream))? {
            make(match meta {
                Meta::All => Meta::All,
                Meta::Alias(alias) => Meta::Alias(alias),
                Meta::Only(Username(name)) => Meta::Only(User(Identifier::Name(name))),
            })
        } else {
            let userspec = if maybe(accept_if(|c| c == '%', stream))?.is_some() {
                let ctor = if maybe(accept_if(|c| c == ':', stream))?.is_some() {
                    UserSpecifier::NonunixGroup
                } else {
                    UserSpecifier::Group
                };
                // in this case we must fail 'hard', since input has been consumed
                ctor(expect_nonterminal(stream)?)
            } else if maybe(accept_if(|c| c == '+', stream))?.is_some() {
                // TODO Netgroups; in this case we need to "return early" since
                // netgroups don't share the syntactic structure of the other alternatives
                unrecoverable!("netgroups are not supported yet");
            } else {
                // in this case we must fail 'softly', since no input has been consumed yet
                UserSpecifier::User(try_nonterminal(stream)?)
            };

            make(Meta::Only(userspec))
        }
    }
}

impl Many for UserSpecifier {}

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

// temporary stubs
fn is_bool_param(_name: &str) -> bool {
    true
}
#[allow(dead_code)]
fn is_int_param(_name: &str) -> bool {
    true
}
#[allow(dead_code)]
fn is_string_param(_name: &str) -> bool {
    true
}
#[allow(dead_code)]
fn is_list_param(_name: &str) -> bool {
    _name != "secure_path"
}

fn get_directive(
    perhaps_keyword: &Spec<UserSpecifier>,
    stream: &mut Peekable<impl Iterator<Item = char>>,
) -> Parsed<Directive> {
    use crate::ast::Directive::*;
    use crate::ast::Meta::*;
    use crate::ast::Qualified::*;
    use crate::ast::UserSpecifier::*;
    let Allow(Only(User(Identifier::Name(keyword)))) = perhaps_keyword else { return reject() };

    /// Parse an alias definition
    fn parse_alias<T>(
        ctor: fn(Def<T>) -> Directive,
        stream: &mut Peekable<impl Iterator<Item = char>>,
    ) -> Parsed<Directive>
    where
        Meta<T>: Parse + Many,
    {
        let Upper(name) = expect_nonterminal(stream)?;
        expect_syntax('=', stream)?;

        make(ctor(Def(name, expect_nonterminal(stream)?)))
    }

    /// Parse multiple entries enclosed in quotes (for list-like Defaults-settings)
    fn parse_vars(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Vec<String>> {
        if try_syntax('"', stream).is_ok() {
            let mut result = Vec::new();
            while let Some(EnvVar(name)) = maybe(try_nonterminal(stream))? {
                result.push(name);
                if try_syntax('=', stream).is_ok() {
                    // TODO
                    let QuotedText(_) = expect_nonterminal(stream)?;
                    expect_syntax('"', stream)?;
                    unrecoverable!("values in environment variables not yet supported")
                }
            }
            expect_syntax('"', stream)?;
            if result.is_empty() {
                unrecoverable!("empty string not allowed");
            }

            make(result)
        } else {
            let EnvVar(name) = expect_nonterminal(stream)?;

            make(vec![name])
        }
    }

    /// Parse "Defaults" entries
    fn parse_default(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Directive> {
        let bool_setting = |name: String, value: bool| {
            // TODO: other types in a boolean context
            if is_bool_param(&name) {
                make(Defaults(name, DefaultValue::Flag(value)))
            } else {
                unrecoverable!("{name} is not a boolean setting");
            }
        };

        let list_items = |mode: Mode, name: String, stream: &mut _| {
            expect_syntax('=', stream)?;
            if !is_list_param(&name) {
                unrecoverable!("{name} is not a list parameter");
            }
            let items = parse_vars(stream)?;

            make(Defaults(name, DefaultValue::List(mode, items)))
        };

        if try_syntax('!', stream).is_ok() {
            let EnvVar(name) = expect_nonterminal(stream)?;
            bool_setting(name, false)
        } else {
            let EnvVar(name) = try_nonterminal(stream)?;

            if try_syntax('+', stream).is_ok() {
                list_items(Mode::Add, name, stream)
            } else if try_syntax('-', stream).is_ok() {
                list_items(Mode::Del, name, stream)
            } else if try_syntax('=', stream).is_ok() {
                if is_list_param(&name) {
                    let items = parse_vars(stream)?;
                    make(Defaults(name, DefaultValue::List(Mode::Set, items)))
                } else {
                    //TODO: what are the precise syntactic considerations for 'string options'?
                    let text = if try_syntax('"', stream).is_ok() {
                        let QuotedText(text) = expect_nonterminal(stream)?;
                        expect_syntax('"', stream)?;
                        text
                    } else {
                        let EnvVar(name) = expect_nonterminal(stream)?;
                        name
                    };
                    make(Defaults(name, DefaultValue::Text(text)))
                }
            } else {
                bool_setting(name, true)
            }
        }
    }

    match keyword.as_str() {
        "User_Alias" => parse_alias(UserAlias, stream),
        "Host_Alias" => parse_alias(HostAlias, stream),
        "Cmnd_Alias" | "Cmd_Alias" => parse_alias(CmndAlias, stream),
        "Runas_Alias" => parse_alias(RunasAlias, stream),
        "Defaults" => parse_default(stream),
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
