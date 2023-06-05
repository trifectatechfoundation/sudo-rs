use crate::ast_names::UserFriendly;
use crate::basic_parser::*;
use crate::tokens::*;

/// The Sudoers file allows negating items with the exclamation mark.
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub enum Qualified<T> {
    Allow(T),
    Forbid(T),
}

/// Type aliases; many items can be replaced by ALL, aliases, and negated.
pub type Spec<T> = Qualified<Meta<T>>;
pub type SpecList<T> = Vec<Spec<T>>;

/// An identifier is a name or a #number
#[cfg_attr(test, derive(Clone, Debug, PartialEq, Eq))]
pub enum Identifier {
    Name(String),
    ID(u32),
}

/// A userspecifier is either a username, or a (non-unix) group name, or netgroup
#[cfg_attr(test, derive(Clone, Debug, PartialEq, Eq))]
pub enum UserSpecifier {
    User(Identifier),
    Group(Identifier),
    NonunixGroup(Identifier),
}

/// The RunAs specification consists of a (possibly empty) list of userspecifiers, followed by a (possibly empty) list of groups.
pub struct RunAs {
    pub users: SpecList<UserSpecifier>,
    pub groups: SpecList<Identifier>,
}

/// Commands in /etc/sudoers can have attributes attached to them, such as NOPASSWD, NOEXEC, ...
#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub struct Tag {
    pub passwd: bool,
    pub cwd: Option<ChDir>,
}

impl Default for Tag {
    fn default() -> Tag {
        Tag {
            passwd: true,
            cwd: None,
        }
    }
}

/// Commands with attached attributes.
pub struct CommandSpec(pub Vec<Modifier>, pub Spec<Command>);

/// The main AST object for one sudoer-permission line
type PairVec<A, B> = Vec<(A, Vec<B>)>;

pub struct PermissionSpec {
    pub users: SpecList<UserSpecifier>,
    pub permissions: PairVec<SpecList<Hostname>, (Option<RunAs>, CommandSpec)>,
}

pub struct Def<T>(pub String, pub SpecList<T>);

/// AST object for directive specifications (aliases, arguments, etc)
pub enum Directive {
    UserAlias(Def<UserSpecifier>),
    HostAlias(Def<Hostname>),
    CmndAlias(Def<Command>),
    RunasAlias(Def<UserSpecifier>),
    Defaults(Vec<(String, ConfigValue)>),
}

pub type TextEnum = sudo_defaults::StrEnum<'static>;

pub enum ConfigValue {
    Flag(bool),
    Text(Option<Box<str>>),
    Num(i128),
    List(Mode, Vec<String>),
    Enum(TextEnum),
}

pub enum Mode {
    Add,
    Set,
    Del,
}

/// The Sudoers file can contain permissions and directives
pub enum Sudo {
    Spec(PermissionSpec),
    Decl(Directive),
    Include(String),
    IncludeDir(String),
    LineComment,
}

/// grammar:
/// ```text
/// identifier = name
///            | #<numerical id>
/// ```

impl Parse for Identifier {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        if accept_if(|c| c == '#', stream).is_ok() {
            let Digits(guid) = expect_nonterminal(stream)?;
            make(Identifier::ID(guid))
        } else {
            let Username(name) = try_nonterminal(stream)?;
            make(Identifier::Name(name))
        }
    }
}

impl Many for Identifier {}

/// grammar:
/// ```text
/// qualified<T> = T | "!", qualified<T>
/// ```
///
/// This computes the correct negation with multiple exclamation marks in the parsing stage so we
/// are not bothered by it later.

impl<T: Parse + UserFriendly> Parse for Qualified<T> {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        if is_syntax('!', stream)? {
            let mut neg = true;
            while is_syntax('!', stream)? {
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

/// Helper function for parsing Meta<T> things where T is not a token

fn parse_meta<T: Parse>(
    stream: &mut impl CharStream,
    embed: impl FnOnce(String) -> T,
) -> Parsed<Meta<T>> {
    if let Some(meta) = try_nonterminal(stream)? {
        make(match meta {
            Meta::All => Meta::All,
            Meta::Alias(alias) => Meta::Alias(alias),
            Meta::Only(Username(name)) => Meta::Only(embed(name)),
        })
    } else {
        make(Meta::Only(T::parse(stream)?))
    }
}

/// Since Identifier is not a token, add the parser for Meta<Identifier>

impl Parse for Meta<Identifier> {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        parse_meta(stream, Identifier::Name)
    }
}

/// grammar:
/// ```text
/// userspec = identifier
///          | %identifier
///          | %:identifier
///          | +netgroup
/// ```
impl Parse for UserSpecifier {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        let userspec = if accept_if(|c| c == '%', stream).is_ok() {
            let ctor = if accept_if(|c| c == ':', stream).is_ok() {
                UserSpecifier::NonunixGroup
            } else {
                UserSpecifier::Group
            };
            // in this case we must fail 'hard', since input has been consumed
            ctor(expect_nonterminal(stream)?)
        } else if accept_if(|c| c == '+', stream).is_ok() {
            // TODO Netgroups
            unrecoverable!(stream, "netgroups are not supported yet");
        } else {
            // in this case we must fail 'softly', since no input has been consumed yet
            UserSpecifier::User(try_nonterminal(stream)?)
        };

        make(userspec)
    }
}

impl Many for UserSpecifier {}

/// UserSpecifier is not a token, implement the parser for Meta<UserSpecifier>
impl Parse for Meta<UserSpecifier> {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        parse_meta(stream, |name| UserSpecifier::User(Identifier::Name(name)))
    }
}

/// grammar:
/// ```text
/// runas = "(", userlist, (":", grouplist?)?, ")"
/// ```
impl Parse for RunAs {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        try_syntax('(', stream)?;
        let users = try_nonterminal(stream).unwrap_or_default();
        let groups = maybe(try_syntax(':', stream).and_then(|_| try_nonterminal(stream)))?
            .unwrap_or_default();
        expect_syntax(')', stream)?;

        make(RunAs { users, groups })
    }
}

/// Implementing the trait Parse for `Meta<flag>`. Wrapped in an own object to avoid
/// conflicting with a potential future generic parse definition for [Meta].
///
/// The reason for combining a parser for these two unrelated categories is that this is one spot
/// where the sudoer grammar isn't nicely LL(1); so at the same place where "NOPASSWD" can appear,
/// we could also see "ALL".
struct MetaOrTag(Meta<Modifier>);

/// A `Modifier` is something that updates the `Tag`.
pub type Modifier = Box<dyn Fn(&mut Tag)>;

// note: at present, "ALL" can be distinguished from a tag using a lookup of 1, since no tag starts with an "A"; but this feels like hanging onto
// the parseability by a thread (although the original sudo also has some ugly parts, like 'sha224' being an illegal user name).
// to be more general, we impl Parse for Meta<Tag> so a future tag like "AFOOBAR" can be added with no problem

impl Parse for MetaOrTag {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        use Meta::*;
        let AliasName(keyword) = try_nonterminal(stream)?;

        let mut switch = |modifier: fn(&mut Tag)| {
            expect_syntax(':', stream)?;
            make(Box::new(modifier))
        };

        let result: Modifier = match keyword.as_str() {
            "PASSWD" => switch(|tag| tag.passwd = true)?,
            "NOPASSWD" => switch(|tag| tag.passwd = false)?,
            "CWD" => {
                expect_syntax('=', stream)?;
                let path: ChDir = expect_nonterminal(stream)?;
                Box::new(move |tag| tag.cwd = Some(path.clone()))
            }
            "ALL" => return make(MetaOrTag(All)),
            alias => return make(MetaOrTag(Alias(alias.to_string()))),
        };

        make(MetaOrTag(Only(result)))
    }
}

/// grammar:
/// ```text
/// commandspec = [tag modifiers]*, command
/// ```

impl Parse for CommandSpec {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        let mut tags = vec![];
        while let Some(MetaOrTag(keyword)) = try_nonterminal(stream)? {
            use Qualified::Allow;
            match keyword {
                Meta::Only(modifier) => tags.push(modifier),
                Meta::All => return make(CommandSpec(tags, Allow(Meta::All))),
                Meta::Alias(name) => return make(CommandSpec(tags, Allow(Meta::Alias(name)))),
            }
            if tags.len() > Identifier::LIMIT {
                unrecoverable!(stream, "too many tags for command specifier")
            }
        }

        let start_pos = stream.get_pos();
        if let Some(Username(keyword)) = try_nonterminal(stream)? {
            if keyword == "sudoedit" {
                // note: special behaviour of forward slashes in wildcards, tread carefully
                unrecoverable!(pos = start_pos, stream, "sudoedit is not yet supported");
            } else if keyword.starts_with("sha") {
                unrecoverable!(
                    pos = start_pos,
                    stream,
                    "digest specifications are not supported"
                )
            } else {
                unrecoverable!(
                    pos = start_pos,
                    stream,
                    "expected command but found {keyword}"
                )
            };
        }

        let cmd: Spec<Command> = expect_nonterminal(stream)?;

        make(CommandSpec(tags, cmd))
    }
}

/// Parsing for a tuple of hostname, runas specifier and commandspec.
/// grammar:
/// ```text
/// (host,runas,commandspec) = hostlist, "=", [runas?, commandspec]+
/// ```

impl Parse for (SpecList<Hostname>, Vec<(Option<RunAs>, CommandSpec)>) {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        let hosts = try_nonterminal(stream)?;
        expect_syntax('=', stream)?;
        let runas_cmds = expect_nonterminal(stream)?;

        make((hosts, runas_cmds))
    }
}

/// A hostname, runas specifier, commandspec combination can occur multiple times in a single
/// sudoer line (seperated by ":")
impl Many for (SpecList<Hostname>, Vec<(Option<RunAs>, CommandSpec)>) {
    const SEP: char = ':';
}

/// Parsing for a tuple of hostname, runas specifier and commandspec.
/// grammar:
/// ```text
/// (runas,commandspec) = runas?, commandspec
/// ```
impl Parse for (Option<RunAs>, CommandSpec) {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        let runas: Option<RunAs> = try_nonterminal(stream)?;
        let cmd = if runas.is_some() {
            expect_nonterminal(stream)?
        } else {
            try_nonterminal(stream)?
        };

        make((runas, cmd))
    }
}

/// A runas specifier, commandspec combination can occur multiple times in a single
/// sudoer line (seperated by ","); there is some ambiguity in the original grammar:
/// commands can also occur multiple times; we parse that here as if they have an omitted
/// "runas" specifier (which has to be placed correctly during the AST analysis phase)
impl Many for (Option<RunAs>, CommandSpec) {}

/// grammar:
/// ```text
/// sudo = permissionspec
///      | Keyword_Alias identifier = identifier_list
///      | Defaults (name [+-]?= ...)+
/// ```
/// There is a syntactical ambiguity in the sudoer Directive and Permission specifications, so we
/// have to parse them 'together' and do a delayed decision on which category we are in.

impl Parse for Sudo {
    // note: original sudo would reject:
    //   "User_Alias, user machine = command"
    // but accept:
    //   "user, User_Alias machine = command"; this does the same
    fn parse(stream: &mut impl CharStream) -> Parsed<Sudo> {
        if accept_if(|c| c == '@', stream).is_ok() {
            return parse_include(stream);
        }

        // the existence of "#include" forces us to handle lines that start with #<ID> explicitly
        if stream.peek() == Some('#') {
            return if let Ok(ident) = try_nonterminal::<Identifier>(stream) {
                let first_user = Qualified::Allow(Meta::Only(UserSpecifier::User(ident)));
                let users = if is_syntax(',', stream)? {
                    // parse the rest of the userlist and add the already-parsed user in front
                    let mut rest = expect_nonterminal::<SpecList<_>>(stream)?;
                    rest.insert(0, first_user);
                    rest
                } else {
                    vec![first_user]
                };
                // no need to check get_directive as no other directive starts with #
                let permissions = expect_nonterminal(stream)?;
                make(Sudo::Spec(PermissionSpec { users, permissions }))
            } else {
                // the failed "try_nonterminal::<Identifier>" will have consumed the '#'
                // the most ignominious part of sudoers: having to parse bits of comments
                parse_include(stream).or_else(|_| {
                    while accept_if(|c| c != '\n', stream).is_ok() {}
                    make(Sudo::LineComment)
                })
            };
        }

        let start_pos = stream.get_pos();
        if let Some(users) = maybe(try_nonterminal::<SpecList<_>>(stream))? {
            // element 1 always exists (parse_list fails on an empty list)
            let key = &users[0];
            if let Some(directive) = maybe(get_directive(key, stream))? {
                if users.len() != 1 {
                    unrecoverable!(pos = start_pos, stream, "invalid user name list");
                }
                make(Sudo::Decl(directive))
            } else {
                let permissions = expect_nonterminal(stream)?;
                make(Sudo::Spec(PermissionSpec { users, permissions }))
            }
        } else {
            // this will leave whatever could not be parsed on the input stream
            make(Sudo::LineComment)
        }
    }
}

/// Parse the include/include dir part that comes after the '#' or '@' prefix symbol

fn parse_include(stream: &mut impl CharStream) -> Parsed<Sudo> {
    fn get_path(stream: &mut impl CharStream) -> Parsed<String> {
        if accept_if(|c| c == '"', stream).is_ok() {
            let QuotedText(path) = expect_nonterminal(stream)?;
            expect_syntax('"', stream)?;
            make(path)
        } else {
            let value_pos = stream.get_pos();
            let IncludePath(path) = expect_nonterminal(stream)?;
            if stream.peek() != Some('\n') {
                unrecoverable!(
                    pos = value_pos,
                    stream,
                    "use quotes around filenames or escape whitespace"
                )
            }
            make(path)
        }
    }

    let key_pos = stream.get_pos();
    let result = match try_nonterminal(stream)? {
        Some(Username(key)) if key == "include" => Sudo::Include(get_path(stream)?),
        Some(Username(key)) if key == "includedir" => Sudo::IncludeDir(get_path(stream)?),
        _ => unrecoverable!(pos = key_pos, stream, "unknown directive"),
    };

    make(result)
}

use sudo_defaults::sudo_default;
use sudo_defaults::SudoDefault as Setting;

fn get_directive(
    perhaps_keyword: &Spec<UserSpecifier>,
    stream: &mut impl CharStream,
) -> Parsed<Directive> {
    use crate::ast::Directive::*;
    use crate::ast::Meta::*;
    use crate::ast::Qualified::*;
    use crate::ast::UserSpecifier::*;
    let Allow(Only(User(Identifier::Name(keyword)))) = perhaps_keyword else { return reject() };

    /// Parse an alias definition
    fn parse_alias<T: UserFriendly>(
        ctor: fn(Def<T>) -> Directive,
        stream: &mut impl CharStream,
    ) -> Parsed<Directive>
    where
        Meta<T>: Parse + Many,
    {
        let AliasName(name) = expect_nonterminal(stream)?;
        expect_syntax('=', stream)?;

        make(ctor(Def(name, expect_nonterminal(stream)?)))
    }

    /// Parse "Defaults" entries
    fn parse_default<T: CharStream>(stream: &mut T) -> Parsed<Directive> {
        let parameters = expect_nonterminal(stream)?;
        make(Defaults(parameters))
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

/// grammar:
/// ```text
/// parameter = name [+-]?= ...
/// ```
impl Parse for (String, ConfigValue) {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        let id_pos = stream.get_pos();

        // Parse multiple entries enclosed in quotes (for list-like Defaults-settings)
        let parse_vars = |stream: &mut _| -> Parsed<Vec<String>> {
            if accept_if(|c| c == '"', stream).is_ok() {
                let mut result = Vec::new();
                while let Some(EnvVar(name)) = try_nonterminal(stream)? {
                    result.push(name);
                    if is_syntax('=', stream)? {
                        let QuotedText(_) = expect_nonterminal(stream)?;
                        expect_syntax('"', stream)?;
                        unrecoverable!(stream, "values in environment variables not yet supported")
                    }
                }
                expect_syntax('"', stream)?;
                if result.is_empty() {
                    unrecoverable!(stream, "empty string not allowed");
                }

                make(result)
            } else {
                let EnvVar(name) = expect_nonterminal(stream)?;

                make(vec![name])
            }
        };

        // Parse the remainder of a list variable
        let list_items = |mode: Mode, name: String, cfg: Setting, stream: &mut _| {
            expect_syntax('=', stream)?;
            if !matches!(cfg, Setting::List(_)) {
                unrecoverable!(pos = id_pos, stream, "{name} is not a list parameter");
            }

            make((name, ConfigValue::List(mode, parse_vars(stream)?)))
        };

        // Parse a text parameter
        let text_item = |stream: &mut _| {
            if accept_if(|c| c == '"', stream).is_ok() {
                let QuotedText(text) = expect_nonterminal(stream)?;
                expect_syntax('"', stream)?;
                make(text)
            } else {
                let StringParameter(name) = expect_nonterminal(stream)?;
                make(name)
            }
        };

        use sudo_defaults::OptTuple;

        if is_syntax('!', stream)? {
            let value_pos = stream.get_pos();
            let EnvVar(name) = expect_nonterminal(stream)?;
            let value = match sudo_default(&name) {
                Some(Setting::Flag(_)) => ConfigValue::Flag(false),
                Some(Setting::List(_)) => ConfigValue::List(Mode::Set, vec![]),
                Some(Setting::Text(OptTuple {
                    negated: Some(val), ..
                })) => ConfigValue::Text(val.map(|x| x.into())),
                Some(Setting::Enum(OptTuple {
                    negated: Some(val), ..
                })) => ConfigValue::Enum(val),
                Some(Setting::Integer(
                    OptTuple {
                        negated: Some(val), ..
                    },
                    _checker,
                )) => ConfigValue::Num(val),
                None => unrecoverable!(pos = value_pos, stream, "unknown setting: '{name}'"),
                _ => unrecoverable!(
                    pos = value_pos,
                    stream,
                    "'{name}' cannot be used in a boolean context"
                ),
            };
            make((name, value))
        } else {
            let EnvVar(name) = try_nonterminal(stream)?;
            let Some(cfg) = sudo_default(&name) else {
                unrecoverable!(pos = id_pos, stream, "unknown setting: '{name}'");
            };

            if is_syntax('+', stream)? {
                list_items(Mode::Add, name, cfg, stream)
            } else if is_syntax('-', stream)? {
                list_items(Mode::Del, name, cfg, stream)
            } else if is_syntax('=', stream)? {
                let value_pos = stream.get_pos();
                match cfg {
                    Setting::Flag(_) => {
                        unrecoverable!(stream, "can't assign to boolean setting '{name}'")
                    }
                    Setting::Integer(_, checker) => {
                        let Numeric(denotation) = expect_nonterminal(stream)?;
                        if let Some(value) = checker(&denotation) {
                            make((name, ConfigValue::Num(value)))
                        } else {
                            unrecoverable!(
                                pos = value_pos,
                                stream,
                                "'{denotation}' is not a valid value for {name}"
                            );
                        }
                    }
                    Setting::List(_) => {
                        let items = parse_vars(stream)?;
                        make((name, ConfigValue::List(Mode::Set, items)))
                    }
                    Setting::Text(_) => {
                        let text = text_item(stream)?;
                        make((name, ConfigValue::Text(Some(text.into_boxed_str()))))
                    }
                    Setting::Enum(sudo_defaults::OptTuple { default: key, .. }) => {
                        let text = text_item(stream)?;
                        let Some(value) = key.alt(&text) else {
                            unrecoverable!(
                                pos = value_pos,
                                stream,
                                "'{text}' is not a valid value for {name}"
                            );
                        };
                        make((name, ConfigValue::Enum(value)))
                    }
                }
            } else {
                if !matches!(cfg, Setting::Flag(_)) {
                    unrecoverable!(pos = id_pos, stream, "'{name}' is not a boolean setting");
                }
                make((name, ConfigValue::Flag(true)))
            }
        }
    }
}

impl Many for (String, ConfigValue) {}
