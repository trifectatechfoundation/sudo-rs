use super::ast_names::UserFriendly;
use super::basic_parser::*;
use super::tokens::*;
use crate::common::SudoString;
use crate::common::{
    HARDENED_ENUM_VALUE_0, HARDENED_ENUM_VALUE_1, HARDENED_ENUM_VALUE_2, HARDENED_ENUM_VALUE_3,
    HARDENED_ENUM_VALUE_4,
};

/// The Sudoers file allows negating items with the exclamation mark.
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
#[repr(u32)]
pub enum Qualified<T> {
    Allow(T) = HARDENED_ENUM_VALUE_0,
    Forbid(T) = HARDENED_ENUM_VALUE_1,
}

impl<T> Qualified<T> {
    pub fn as_ref(&self) -> Qualified<&T> {
        match self {
            Qualified::Allow(item) => Qualified::Allow(item),
            Qualified::Forbid(item) => Qualified::Forbid(item),
        }
    }

    pub fn negate(&self) -> Qualified<&T> {
        match self {
            Qualified::Allow(item) => Qualified::Forbid(item),
            Qualified::Forbid(item) => Qualified::Allow(item),
        }
    }

    #[cfg(test)]
    pub fn as_allow(&self) -> Option<&T> {
        if let Self::Allow(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

/// Type aliases; many items can be replaced by ALL, aliases, and negated.
pub type Spec<T> = Qualified<Meta<T>>;
pub type SpecList<T> = Vec<Spec<T>>;

/// An identifier is a name or a #number
#[cfg_attr(test, derive(Clone, Debug, PartialEq, Eq))]
#[repr(u32)]
pub enum Identifier {
    Name(SudoString) = HARDENED_ENUM_VALUE_0,
    ID(u32) = HARDENED_ENUM_VALUE_1,
}

/// A userspecifier is either a username, or a (non-unix) group name, or netgroup
#[cfg_attr(test, derive(Clone, Debug, PartialEq, Eq))]
#[repr(u32)]
pub enum UserSpecifier {
    User(Identifier) = HARDENED_ENUM_VALUE_0,
    Group(Identifier) = HARDENED_ENUM_VALUE_1,
    NonunixGroup(Identifier) = HARDENED_ENUM_VALUE_2,
}

/// The RunAs specification consists of a (possibly empty) list of userspecifiers, followed by a (possibly empty) list of groups.
pub struct RunAs {
    pub users: SpecList<UserSpecifier>,
    pub groups: SpecList<Identifier>,
}

// `sudo -l l` calls this the `authenticate` option
#[derive(Copy, Clone, Default, PartialEq)]
#[cfg_attr(test, derive(Debug, Eq))]
#[repr(u32)]
pub enum Authenticate {
    #[default]
    None = HARDENED_ENUM_VALUE_0,
    // PASSWD:
    Passwd = HARDENED_ENUM_VALUE_1,
    // NOPASSWD:
    Nopasswd = HARDENED_ENUM_VALUE_2,
}

/// Commands in /etc/sudoers can have attributes attached to them, such as NOPASSWD, NOEXEC, ...
#[derive(Default, Clone, PartialEq)]
#[cfg_attr(test, derive(Debug, Eq))]
pub struct Tag {
    pub authenticate: Authenticate,
    pub cwd: Option<ChDir>,
}

impl Tag {
    pub fn needs_passwd(&self) -> bool {
        matches!(self.authenticate, Authenticate::None | Authenticate::Passwd)
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

pub type Defs<T> = Vec<Def<T>>;
pub struct Def<T>(pub String, pub SpecList<T>);

/// AST object for directive specifications (aliases, arguments, etc)
#[repr(u32)]
pub enum Directive {
    UserAlias(Defs<UserSpecifier>) = HARDENED_ENUM_VALUE_0,
    HostAlias(Defs<Hostname>) = HARDENED_ENUM_VALUE_1,
    CmndAlias(Defs<Command>) = HARDENED_ENUM_VALUE_2,
    RunasAlias(Defs<UserSpecifier>) = HARDENED_ENUM_VALUE_3,
    Defaults(Vec<(String, ConfigValue)>) = HARDENED_ENUM_VALUE_4,
}

pub type TextEnum = crate::defaults::StrEnum<'static>;

#[repr(u32)]
pub enum ConfigValue {
    Flag(bool) = HARDENED_ENUM_VALUE_0,
    Text(Option<Box<str>>) = HARDENED_ENUM_VALUE_1,
    Num(i64) = HARDENED_ENUM_VALUE_2,
    List(Mode, Vec<String>) = HARDENED_ENUM_VALUE_3,
    Enum(TextEnum) = HARDENED_ENUM_VALUE_4,
}

#[repr(u32)]
pub enum Mode {
    Add = HARDENED_ENUM_VALUE_0,
    Set = HARDENED_ENUM_VALUE_1,
    Del = HARDENED_ENUM_VALUE_2,
}

/// The Sudoers file can contain permissions and directives
#[repr(u32)]
pub enum Sudo {
    Spec(PermissionSpec) = HARDENED_ENUM_VALUE_0,
    Decl(Directive) = HARDENED_ENUM_VALUE_1,
    Include(String) = HARDENED_ENUM_VALUE_2,
    IncludeDir(String) = HARDENED_ENUM_VALUE_3,
    LineComment = HARDENED_ENUM_VALUE_4,
}

impl Sudo {
    #[cfg(test)]
    pub fn is_spec(&self) -> bool {
        matches!(self, Self::Spec(..))
    }

    #[cfg(test)]
    pub fn is_decl(&self) -> bool {
        matches!(self, Self::Decl(..))
    }

    #[cfg(test)]
    pub fn is_line_comment(&self) -> bool {
        matches!(self, Self::LineComment)
    }

    #[cfg(test)]
    pub fn is_include(&self) -> bool {
        matches!(self, Self::Include(..))
    }

    #[cfg(test)]
    pub fn is_include_dir(&self) -> bool {
        matches!(self, Self::IncludeDir(..))
    }

    #[cfg(test)]
    pub fn as_include(&self) -> &str {
        if let Self::Include(v) = self {
            v
        } else {
            panic!()
        }
    }

    #[cfg(test)]
    pub fn as_spec(&self) -> Option<&PermissionSpec> {
        if let Self::Spec(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

/// grammar:
/// ```text
/// identifier = name
///            | #<numerical id>
/// ```
impl Parse for Identifier {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        if accept_if(|c| c == '#', stream).is_some() {
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

/// Helper function for parsing `Meta<T>` things where T is not a token
fn parse_meta<T: Parse>(
    stream: &mut impl CharStream,
    embed: impl FnOnce(SudoString) -> T,
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

/// Since Identifier is not a token, add the parser for `Meta<Identifier>`
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
        let userspec = if accept_if(|c| c == '%', stream).is_some() {
            let ctor = if accept_if(|c| c == ':', stream).is_some() {
                UserSpecifier::NonunixGroup
            } else {
                UserSpecifier::Group
            };
            // in this case we must fail 'hard', since input has been consumed
            ctor(expect_nonterminal(stream)?)
        } else if accept_if(|c| c == '+', stream).is_some() {
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

/// UserSpecifier is not a token, implement the parser for `Meta<UserSpecifier>`
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
            "PASSWD" => switch(|tag| tag.authenticate = Authenticate::Passwd)?,
            "NOPASSWD" => switch(|tag| tag.authenticate = Authenticate::Nopasswd)?,
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
            } else if keyword == "list" {
                unrecoverable!(pos = start_pos, stream, "list is not yet supported");
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
        if accept_if(|c| c == '@', stream).is_some() {
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
                    while accept_if(|c| c != '\n', stream).is_some() {}
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
        if accept_if(|c| c == '"', stream).is_some() {
            let QuotedInclude(path) = expect_nonterminal(stream)?;
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

use crate::defaults::sudo_default;
use crate::defaults::SudoDefault as Setting;

/// grammar:
/// ```text
/// name = definition [ : name = definiton [ : ... ] ]
/// ```
///
impl<T> Parse for Def<T>
where
    T: UserFriendly,
    Meta<T>: Parse + Many,
{
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        let begin_pos = stream.get_pos();
        let AliasName(name) = try_nonterminal(stream)?;
        if name == "ALL" {
            unrecoverable!(
                pos = begin_pos,
                stream,
                "the reserved alias ALL cannot be redefined"
            );
        }
        expect_syntax('=', stream)?;

        make(Def(name, expect_nonterminal(stream)?))
    }
}

impl<T> Many for Def<T> {
    const SEP: char = ':';
}

fn get_directive(
    perhaps_keyword: &Spec<UserSpecifier>,
    stream: &mut impl CharStream,
) -> Parsed<Directive> {
    use super::ast::Directive::*;
    use super::ast::Meta::*;
    use super::ast::Qualified::*;
    use super::ast::UserSpecifier::*;
    let Allow(Only(User(Identifier::Name(keyword)))) = perhaps_keyword else {
        return reject();
    };

    match keyword.as_str() {
        "User_Alias" => make(UserAlias(expect_nonterminal(stream)?)),
        "Host_Alias" => make(HostAlias(expect_nonterminal(stream)?)),
        "Cmnd_Alias" | "Cmd_Alias" => make(CmndAlias(expect_nonterminal(stream)?)),
        "Runas_Alias" => make(RunasAlias(expect_nonterminal(stream)?)),
        "Defaults" => make(Defaults(expect_nonterminal(stream)?)),
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
            if accept_if(|c| c == '"', stream).is_some() {
                let mut result = Vec::new();
                while let Some(EnvVar(name)) = try_nonterminal(stream)? {
                    result.push(name);
                    if is_syntax('=', stream)? {
                        let EnvVar(_) = expect_nonterminal(stream)?;
                        unrecoverable!(stream, "values in environment variables not yet supported")
                    }
                    if result.len() > Identifier::LIMIT {
                        unrecoverable!(stream, "environment variable list too long")
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
            if accept_if(|c| c == '"', stream).is_some() {
                let QuotedText(text) = expect_nonterminal(stream)?;
                expect_syntax('"', stream)?;
                make(text)
            } else {
                let StringParameter(name) = expect_nonterminal(stream)?;
                make(name)
            }
        };

        use crate::defaults::OptTuple;

        if is_syntax('!', stream)? {
            let value_pos = stream.get_pos();
            let DefaultName(name) = expect_nonterminal(stream)?;
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
            let DefaultName(name) = try_nonterminal(stream)?;
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
                    Setting::Enum(OptTuple { default: key, .. }) => {
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
