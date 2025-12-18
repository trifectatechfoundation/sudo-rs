use super::ast_names::UserFriendly;
use super::basic_parser::*;
use super::char_stream::advance;
use super::tokens::*;
use crate::common::SudoString;
use crate::common::{
    HARDENED_ENUM_VALUE_0, HARDENED_ENUM_VALUE_1, HARDENED_ENUM_VALUE_2, HARDENED_ENUM_VALUE_3,
    HARDENED_ENUM_VALUE_4,
};
use crate::defaults;

/// The Sudoers file allows negating items with the exclamation mark.
#[cfg_attr(test, derive(Debug, Eq))]
#[derive(Clone, PartialEq)]
#[repr(u32)]
pub enum Qualified<T> {
    Allow(T) = HARDENED_ENUM_VALUE_0,
    Forbid(T) = HARDENED_ENUM_VALUE_1,
}

/// Type aliases; many items can be replaced by ALL, aliases, and negated.
pub type Spec<T> = Qualified<Meta<T>>;
pub type SpecList<T> = Vec<Spec<T>>;

/// A generic mapping function (only used for turning `Spec<SimpleCommand>` into `Spec<Command>`)
impl<T> Spec<T> {
    pub fn map<U>(self, f: impl Fn(T) -> U) -> Spec<U> {
        let transform = |meta| match meta {
            Meta::All => Meta::All,
            Meta::Alias(alias) => Meta::Alias(alias),
            Meta::Only(x) => Meta::Only(f(x)),
        };

        match self {
            Qualified::Allow(x) => Qualified::Allow(transform(x)),
            Qualified::Forbid(x) => Qualified::Forbid(transform(x)),
        }
    }
}

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

#[derive(Copy, Clone, Default, PartialEq)]
#[cfg_attr(test, derive(Debug, Eq))]
#[repr(u32)]
pub enum EnvironmentControl {
    #[default]
    Implicit = HARDENED_ENUM_VALUE_0,
    // PASSWD:
    Setenv = HARDENED_ENUM_VALUE_1,
    // NOPASSWD:
    Nosetenv = HARDENED_ENUM_VALUE_2,
}

#[derive(Copy, Clone, Default, PartialEq)]
#[cfg_attr(test, derive(Debug, Eq))]
#[repr(u32)]
pub enum ExecControl {
    #[default]
    Implicit = HARDENED_ENUM_VALUE_0,
    // PASSWD:
    Exec = HARDENED_ENUM_VALUE_1,
    // NOPASSWD:
    Noexec = HARDENED_ENUM_VALUE_2,
}

/// Commands in /etc/sudoers can have attributes attached to them, such as NOPASSWD, NOEXEC, ...
#[derive(Default, Clone, PartialEq)]
#[cfg_attr(test, derive(Debug, Eq))]
pub struct Tag {
    pub(super) authenticate: Authenticate,
    pub(super) cwd: Option<ChDir>,
    pub(super) env: EnvironmentControl,
    pub(super) apparmor_profile: Option<String>,
    pub(super) noexec: ExecControl,
    pub(super) ignored: Vec<Span>,
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
    Defaults(Vec<defaults::SettingsModifier>, ConfigScope) = HARDENED_ENUM_VALUE_4,
}

/// AST object for the 'context' (host, user, cmnd, runas) of a Defaults directive
#[repr(u32)]
pub enum ConfigScope {
    // "Defaults entries are parsed in the following order:
    // generic, host and user Defaults first, then runas Defaults and finally command defaults."
    Generic = HARDENED_ENUM_VALUE_0,
    Host(SpecList<Hostname>) = HARDENED_ENUM_VALUE_1,
    User(SpecList<UserSpecifier>) = HARDENED_ENUM_VALUE_2,
    RunAs(SpecList<UserSpecifier>) = HARDENED_ENUM_VALUE_3,
    Command(SpecList<SimpleCommand>) = HARDENED_ENUM_VALUE_4,
}

/// The Sudoers file can contain permissions and directives
#[repr(u32)]
pub enum Sudo {
    Spec(PermissionSpec) = HARDENED_ENUM_VALUE_0,
    Decl(Directive) = HARDENED_ENUM_VALUE_1,
    Include(String, Span) = HARDENED_ENUM_VALUE_2,
    IncludeDir(String, Span) = HARDENED_ENUM_VALUE_3,
    LineComment = HARDENED_ENUM_VALUE_4,
}

/// grammar:
/// ```text
/// identifier = name
///            | #<numerical id>
/// ```
impl Parse for Identifier {
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
        if stream.eat_char('#') {
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
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
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
    stream: &mut CharStream,
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
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
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
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
        fn parse_user(stream: &mut CharStream) -> Parsed<UserSpecifier> {
            let userspec = if stream.eat_char('%') {
                let ctor = if stream.eat_char(':') {
                    UserSpecifier::NonunixGroup
                } else {
                    UserSpecifier::Group
                };
                // in this case we must fail 'hard', since input has been consumed
                ctor(expect_nonterminal(stream)?)
            } else if stream.eat_char('+') {
                // TODO Netgroups
                unrecoverable!(stream, "netgroups are not supported yet");
            } else {
                // in this case we must fail 'softly', since no input has been consumed yet
                UserSpecifier::User(try_nonterminal(stream)?)
            };

            make(userspec)
        }

        // if we see a quote, first parse the quoted text as a token and then
        // re-parse whatever we found inside; this is a lazy solution but it works
        if stream.eat_char('"') {
            let begin_pos = stream.get_pos();
            let Unquoted(text, _): Unquoted<Username> = expect_nonterminal(stream)?;
            let result = parse_user(&mut CharStream::new_with_pos(&text, begin_pos))?;
            expect_syntax('"', stream)?;

            Ok(result)
        } else {
            parse_user(stream)
        }
    }
}

impl Many for UserSpecifier {}

/// UserSpecifier is not a token, implement the parser for `Meta<UserSpecifier>`
impl Parse for Meta<UserSpecifier> {
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
        parse_meta(stream, |name| UserSpecifier::User(Identifier::Name(name)))
    }
}

/// grammar:
/// ```text
/// runas = "(", userlist, (":", grouplist?)?, ")"
/// ```
impl Parse for RunAs {
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
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
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
        use Meta::*;

        let start_pos = stream.get_pos();
        let AliasName(keyword) = try_nonterminal(stream)?;

        let mut switch = |modifier: fn(&mut Tag)| {
            expect_syntax(':', stream)?;
            make(Box::new(modifier))
        };

        let result: Modifier = match keyword.as_str() {
            "EXEC" => switch(|tag| tag.noexec = ExecControl::Exec)?,
            "NOEXEC" => switch(|tag| tag.noexec = ExecControl::Noexec)?,

            "SETENV" => switch(|tag| tag.env = EnvironmentControl::Setenv)?,
            "NOSETENV" => switch(|tag| tag.env = EnvironmentControl::Nosetenv)?,
            "PASSWD" => switch(|tag| tag.authenticate = Authenticate::Passwd)?,
            "NOPASSWD" => switch(|tag| tag.authenticate = Authenticate::Nopasswd)?,

            "CWD" => {
                expect_syntax('=', stream)?;
                let path: ChDir = expect_nonterminal(stream)?;
                Box::new(move |tag| tag.cwd = Some(path.clone()))
            }

            // we do not support these, and that should make sudo-rs "fail safe"
            spec @ ("INTERCEPT" | "CHROOT" | "TIMEOUT" | "NOTBEFORE" | "NOTAFTER") => {
                unrecoverable!(
                    pos = start_pos,
                    stream,
                    "{spec} is not supported by sudo-rs"
                )
            }
            "ROLE" | "TYPE" => unrecoverable!(
                pos = start_pos,
                stream,
                "SELinux role based access control is not yet supported by sudo-rs"
            ),

            // this is less fatal
            "LOG_INPUT" | "NOLOG_INPUT" | "LOG_OUTPUT" | "NOLOG_OUTPUT" | "MAIL" | "NOMAIL"
            | "FOLLOW" => {
                let ignored_location = Span {
                    start: start_pos,
                    end: stream.get_pos(),
                };
                expect_syntax(':', stream)?;
                Box::new(move |tag| tag.ignored.push(ignored_location))
            }

            // 'NOFOLLOW' and 'NOINTERCEPT' are the default behaviour.
            "NOFOLLOW" | "NOINTERCEPT" => switch(|_| {})?,

            "APPARMOR_PROFILE" => {
                expect_syntax('=', stream)?;
                let StringParameter(profile) = expect_nonterminal(stream)?;
                Box::new(move |tag| tag.apparmor_profile = Some(profile.clone()))
            }

            "ALL" => return make(MetaOrTag(All)),
            alias => {
                if is_syntax('=', stream)? {
                    unrecoverable!(pos = start_pos, stream, "unsupported modifier '{}'", alias);
                } else {
                    return make(MetaOrTag(Alias(alias.to_string())));
                }
            }
        };

        make(MetaOrTag(Only(result)))
    }
}

/// grammar:
/// ```text
/// commandspec = [tag modifiers]*, command
/// ```
impl Parse for CommandSpec {
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
        use Qualified::Allow;
        let mut tags = vec![];
        while let Some(MetaOrTag(keyword)) = try_nonterminal(stream)? {
            match keyword {
                Meta::Only(modifier) => tags.push(modifier),
                Meta::All => return make(CommandSpec(tags, Allow(Meta::All))),
                Meta::Alias(name) => return make(CommandSpec(tags, Allow(Meta::Alias(name)))),
            }
            if tags.len() > Identifier::LIMIT {
                unrecoverable!(stream, "too many tags for command specifier")
            }
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
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
        let hosts = try_nonterminal(stream)?;
        expect_syntax('=', stream)?;
        let runas_cmds = expect_nonterminal(stream)?;

        make((hosts, runas_cmds))
    }
}

/// A hostname, runas specifier, commandspec combination can occur multiple times in a single
/// sudoer line (separated by ":")
impl Many for (SpecList<Hostname>, Vec<(Option<RunAs>, CommandSpec)>) {
    const SEP: char = ':';
}

/// Parsing for a tuple of hostname, runas specifier and commandspec.
/// grammar:
/// ```text
/// (runas,commandspec) = runas?, commandspec
/// ```
impl Parse for (Option<RunAs>, CommandSpec) {
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
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
/// sudoer line (separated by ","); there is some ambiguity in the original grammar:
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
    fn parse(stream: &mut CharStream) -> Parsed<Sudo> {
        if stream.eat_char('@') {
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
                    stream.skip_to_newline();
                    make(Sudo::LineComment)
                })
            };
        }

        let start_pos = stream.get_pos();
        if stream.peek() == Some('"') {
            // a quoted userlist follows; this forces us to read a userlist
            let users = expect_nonterminal(stream)?;
            let permissions = expect_nonterminal(stream)?;
            make(Sudo::Spec(PermissionSpec { users, permissions }))
        } else if let Some(users) = maybe(try_nonterminal::<SpecList<_>>(stream))? {
            // this could be the start of a Defaults or Alias definition, so distinguish.
            // element 1 always exists (parse_list fails on an empty list)
            let key = &users[0];
            if let Some(directive) = maybe(get_directive(key, stream, start_pos))? {
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
fn parse_include(stream: &mut CharStream) -> Parsed<Sudo> {
    fn get_path(stream: &mut CharStream, key_pos: (usize, usize)) -> Parsed<(String, Span)> {
        let path = if stream.eat_char('"') {
            let QuotedIncludePath(path) = expect_nonterminal(stream)?;
            expect_syntax('"', stream)?;
            path
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
            path
        };
        make((
            path,
            Span {
                start: key_pos,
                end: stream.get_pos(),
            },
        ))
    }

    let key_pos = stream.get_pos();
    let result = match try_nonterminal(stream)? {
        Some(Username(key)) if key == "include" => {
            let (path, span) = get_path(stream, key_pos)?;
            Sudo::Include(path, span)
        }
        Some(Username(key)) if key == "includedir" => {
            let (path, span) = get_path(stream, key_pos)?;
            Sudo::IncludeDir(path, span)
        }
        _ => unrecoverable!(pos = key_pos, stream, "unknown directive"),
    };

    make(result)
}

/// grammar:
/// ```text
/// name = definition [ : name = definition [ : ... ] ]
/// ```
///
impl<T> Parse for Def<T>
where
    T: UserFriendly,
    Meta<T>: Parse + Many,
{
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
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

// NOTE: This function is a bit of a hack, since it relies on the fact that all directives
// occur in the spot of a username, and are of a form that would otherwise be a legal user name.
// I.e. after a valid username has been parsed, we check if it isn't actually a valid start of a
// directive. A more robust solution would be to use the approach taken by the `MetaOrTag` above.

fn get_directive(
    perhaps_keyword: &Spec<UserSpecifier>,
    stream: &mut CharStream,
    begin_pos: (usize, usize),
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
        _ if keyword.starts_with("Defaults") => {
            //HACK #1: no space is allowed between "Defaults" and '!>@:'. The below avoids having to
            //add "Defaults!" etc as separate tokens; but relying on positional information during
            //parsing is of course, cheating.
            //HACK #2: '@' can be part of a username, so it will already have been parsed;
            //an acceptable hostname is subset of an acceptable username, so that's actually OK.
            //This resolves an ambiguity in the grammar similarly to how MetaOrTag does that.
            const DEFAULTS_LEN: usize = "Defaults".len();
            let allow_scope_modifier = stream.get_pos().0 == begin_pos.0
                && (stream.get_pos().1 - begin_pos.1 == DEFAULTS_LEN
                    || keyword.len() > DEFAULTS_LEN);

            let scope = if allow_scope_modifier {
                if keyword[DEFAULTS_LEN..].starts_with('@') {
                    let inner_stream = &mut CharStream::new_with_pos(
                        &keyword[DEFAULTS_LEN + 1..],
                        advance(begin_pos, DEFAULTS_LEN + 1),
                    );

                    ConfigScope::Host(expect_nonterminal(inner_stream)?)
                } else if is_syntax(':', stream)? {
                    ConfigScope::User(expect_nonterminal(stream)?)
                } else if is_syntax('!', stream)? {
                    ConfigScope::Command(expect_nonterminal(stream)?)
                } else if is_syntax('>', stream)? {
                    ConfigScope::RunAs(expect_nonterminal(stream)?)
                } else {
                    ConfigScope::Generic
                }
            } else {
                ConfigScope::Generic
            };

            make(Defaults(expect_nonterminal(stream)?, scope))
        }
        _ => reject(),
    }
}

/// grammar:
/// ```text
/// parameter = name [+-]?= ...
/// ```
impl Parse for defaults::SettingsModifier {
    fn parse(stream: &mut CharStream) -> Parsed<Self> {
        let id_pos = stream.get_pos();

        // Parse multiple entries enclosed in quotes (for list-like Defaults-settings)
        let parse_vars = |stream: &mut CharStream| -> Parsed<Vec<String>> {
            if stream.eat_char('"') {
                let mut result = Vec::new();
                while let Some(EnvVar(name)) = try_nonterminal(stream)? {
                    if is_syntax('=', stream)? {
                        let StringParameter(value) = expect_nonterminal(stream)?;
                        result.push(name + "=" + &value);
                    } else {
                        result.push(name);
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
                if is_syntax('=', stream)? {
                    unrecoverable!(stream, "double quotes are required for VAR=value pairs")
                } else {
                    make(vec![name])
                }
            }
        };

        // Parse the remainder of a list variable
        let list_items =
            |mode: defaults::ListMode, name: String, cfg: defaults::SettingKind, stream: &mut _| {
                expect_syntax('=', stream)?;
                let defaults::SettingKind::List(checker) = cfg else {
                    unrecoverable!(pos = id_pos, stream, "{name} is not a list parameter");
                };

                make(checker(mode, parse_vars(stream)?))
            };

        // Parse a text parameter
        let text_item = |stream: &mut CharStream| {
            if stream.eat_char('"') {
                let QuotedStringParameter(text) = expect_nonterminal(stream)?;
                expect_syntax('"', stream)?;
                make(text)
            } else {
                let StringParameter(name) = expect_nonterminal(stream)?;
                make(name)
            }
        };

        if is_syntax('!', stream)? {
            let value_pos = stream.get_pos();
            let DefaultName(name) = expect_nonterminal(stream)?;
            let Some(modifier) = defaults::negate(&name) else {
                if defaults::set(&name).is_some() {
                    unrecoverable!(
                        pos = value_pos,
                        stream,
                        "'{name}' cannot be used in a boolean context"
                    );
                } else {
                    unrecoverable!(pos = value_pos, stream, "unknown setting: '{name}'");
                }
            };

            make(modifier)
        } else {
            let DefaultName(name) = try_nonterminal(stream)?;
            let Some(cfg) = defaults::set(&name) else {
                unrecoverable!(pos = id_pos, stream, "unknown setting: '{name}'");
            };

            if is_syntax('+', stream)? {
                list_items(defaults::ListMode::Add, name, cfg, stream)
            } else if is_syntax('-', stream)? {
                list_items(defaults::ListMode::Del, name, cfg, stream)
            } else if is_syntax('=', stream)? {
                let value_pos = stream.get_pos();
                match cfg {
                    defaults::SettingKind::Flag(_) => {
                        unrecoverable!(stream, "can't assign to boolean setting '{name}'")
                    }
                    defaults::SettingKind::Integer(checker) => {
                        let Numeric(denotation) = expect_nonterminal(stream)?;
                        if let Some(modifier) = checker(&denotation) {
                            make(modifier)
                        } else {
                            unrecoverable!(
                                pos = value_pos,
                                stream,
                                "'{denotation}' is not a valid value for {name}"
                            );
                        }
                    }
                    defaults::SettingKind::List(checker) => {
                        let items = parse_vars(stream)?;

                        make(checker(defaults::ListMode::Set, items))
                    }
                    defaults::SettingKind::Text(checker) => {
                        let text = text_item(stream)?;
                        let Some(modifier) = checker(&text) else {
                            unrecoverable!(
                                pos = value_pos,
                                stream,
                                "'{text}' is not a valid value for {name}"
                            );
                        };
                        make(modifier)
                    }
                }
            } else {
                let defaults::SettingKind::Flag(modifier) = cfg else {
                    unrecoverable!(pos = id_pos, stream, "'{name}' is not a boolean setting");
                };

                make(modifier)
            }
        }
    }
}

impl Many for defaults::SettingsModifier {}
