#![forbid(unsafe_code)]

//! Code that checks (and in the future: lists) permissions in the sudoers file

mod ast;
mod ast_names;
mod basic_parser;
mod char_stream;
mod entry;
mod tokens;

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use crate::common::resolve::{is_valid_executable, resolve_path};
use crate::defaults;
use crate::log::auth_warn;
use crate::system::interface::{GroupId, UnixGroup, UnixUser, UserId};
use crate::system::{self, audit};
use ast::*;
use tokens::*;

pub type Settings = defaults::Settings;
pub use basic_parser::Span;

/// How many nested include files do we allow?
const INCLUDE_LIMIT: u8 = 128;

/// Export some necessary symbols from modules
pub struct Error {
    pub source: Option<PathBuf>,
    pub location: Option<basic_parser::Span>,
    pub message: String,
}

/// A "Customiser" represents a "Defaults" setting that has 'late binding'; i.e.
/// cannot be determined simply by reading a sudoers configuration. This is used
/// for Defaults@host, Defaults:user, Defaults>runas and Defaults!cmd.
///
/// I.e. the Setting modifications in the second part of the tuple only apply for
/// items explicitly matched by the first part of the tuple.
type Customiser<Scope> = (Scope, Vec<defaults::SettingsModifier>);

#[derive(Default)]
pub struct Sudoers {
    rules: Vec<PermissionSpec>,
    aliases: AliasTable,
    settings: Settings,
    customisers: CustomiserTable,
}

/// A structure that represents what the user wants to do
pub struct Request<'a, User: UnixUser, Group: UnixGroup> {
    pub user: &'a User,
    pub group: &'a Group,
    pub command: &'a Path,
    pub arguments: &'a [OsString],
}

pub struct ListRequest<'a, User: UnixUser, Group: UnixGroup> {
    pub inspected_user: &'a User,
    pub target_user: &'a User,
    pub target_group: &'a Group,
}

#[derive(Default)]
#[cfg_attr(test, derive(Clone))]
pub struct Judgement {
    flags: Option<Tag>,
    settings: Settings,
}

mod policy;

pub use policy::{AuthenticatingUser, Authentication, Authorization, DirChange, Restrictions};

pub use self::entry::Entry;

type MatchedCommand<'a> = (Option<&'a RunAs>, (Tag, &'a Spec<Command>));

/// This function takes a file argument for a sudoers file and processes it.
impl Sudoers {
    pub fn open(path: impl AsRef<Path>) -> Result<(Sudoers, Vec<Error>), io::Error> {
        let sudoers = open_sudoers(path.as_ref())?;
        Ok(analyze(path.as_ref(), sudoers))
    }

    pub fn read<R: io::Read, P: AsRef<Path>>(
        reader: R,
        path: P,
    ) -> Result<(Sudoers, Vec<Error>), io::Error> {
        let sudoers = read_sudoers(reader)?;
        Ok(analyze(path.as_ref(), sudoers))
    }

    fn specify_host_user_runas<User: UnixUser + PartialEq<User>>(
        &mut self,
        hostname: &system::Hostname,
        requesting_user: &User,
        target_user: Option<&User>,
    ) {
        let customisers = std::mem::take(&mut self.customisers.non_cmnd);

        let host_matcher = &match_token(hostname);
        let host_aliases = get_aliases(&self.aliases.host, host_matcher);

        let user_matcher = &match_user(requesting_user);
        let user_aliases = get_aliases(&self.aliases.user, user_matcher);

        let runas_matcher_aliases = target_user.map(|target_user| {
            let runas_matcher = match_user(target_user);
            let runas_aliases = get_aliases(&self.aliases.runas, &runas_matcher);

            (runas_matcher, runas_aliases)
        });

        let match_scope = |scope| match scope {
            ConfigScope::Generic => true,
            ConfigScope::Host(list) => find_item(&list, host_matcher, &host_aliases).is_some(),
            ConfigScope::User(list) => find_item(&list, user_matcher, &user_aliases).is_some(),
            ConfigScope::RunAs(list) => {
                runas_matcher_aliases
                    .as_ref()
                    .is_some_and(|(runas_matcher, runas_aliases)| {
                        find_item(&list, runas_matcher, runas_aliases).is_some()
                    })
            }
            ConfigScope::Command(_list) => {
                unreachable!("command-specific defaults are filtered out")
            }
        };

        for (scope, modifiers) in customisers {
            if match_scope(scope) {
                for modifier in modifiers {
                    modifier(&mut self.settings);
                }
            }
        }
    }

    fn specify_command(&mut self, command: &Path, arguments: &[OsString]) {
        let customisers = std::mem::take(&mut self.customisers.cmnd);

        let cmnd_matcher = &match_command((command, arguments));
        let cmnd_aliases = get_aliases(&self.aliases.cmnd, cmnd_matcher);

        for (scope, modifiers) in customisers {
            if find_item(&scope, cmnd_matcher, &cmnd_aliases).is_some() {
                for modifier in modifiers {
                    modifier(&mut self.settings);
                }
            }
        }
    }

    pub fn check<User: UnixUser + PartialEq<User>, Group: UnixGroup>(
        &mut self,
        am_user: &User,
        on_host: &system::Hostname,
        request: Request<User, Group>,
    ) -> Judgement {
        self.specify_host_user_runas(on_host, am_user, Some(request.user));
        self.specify_command(request.command, request.arguments);

        // exception: if user is root or does not switch users, NOPASSWD is implied
        let skip_passwd =
            am_user.is_root() || (request.user == am_user && in_group(am_user, request.group));

        let mut flags = check_permission(self, am_user, on_host, request);
        if let Some(Tag { authenticate, .. }) = flags.as_mut() {
            if skip_passwd {
                *authenticate = Authenticate::Nopasswd;
            }
        }

        Judgement {
            flags,
            settings: self.settings.clone(),
        }
    }

    pub fn check_list_permission<User: UnixUser + PartialEq<User>, Group: UnixGroup>(
        &mut self,
        invoking_user: &User,
        hostname: &system::Hostname,
        request: ListRequest<User, Group>,
    ) -> Authorization {
        let skip_passwd;
        let mut flags = if request.inspected_user != invoking_user {
            skip_passwd = invoking_user.is_root();

            self.check(
                invoking_user,
                hostname,
                Request {
                    user: request.inspected_user,
                    group: &request.inspected_user.group(),
                    command: Path::new("list"),
                    arguments: &[],
                },
            )
            .flags
            .or(invoking_user.is_root().then(Tag::default))
        } else {
            skip_passwd = invoking_user.is_root()
                || (request.target_user == invoking_user
                    && in_group(invoking_user, request.target_group));

            self.matching_user_specs(invoking_user, hostname)
                .flatten()
                .map(|(_, (tag, _))| tag)
                .max_by_key(|tag| !tag.needs_passwd())
        };

        if let Some(tag) = flags.as_mut() {
            if skip_passwd {
                tag.authenticate = Authenticate::Nopasswd;
            }

            Authorization::Allowed(self.settings.to_auth(tag), ())
        } else {
            Authorization::Forbidden
        }
    }

    pub fn check_validate_permission<User: UnixUser + PartialEq<User>>(
        &mut self,
        invoking_user: &User,
        hostname: &system::Hostname,
    ) -> Authorization {
        self.specify_host_user_runas(hostname, invoking_user, None);

        // exception: if user is root, NOPASSWD is implied
        let skip_passwd = invoking_user.is_root();

        let mut flags = self
            .matching_user_specs(invoking_user, hostname)
            .flatten()
            .map(|(_, (tag, _))| tag)
            .max_by_key(|tag| tag.needs_passwd());

        if let Some(tag) = flags.as_mut() {
            if skip_passwd {
                tag.authenticate = Authenticate::Nopasswd;
            }

            Authorization::Allowed(self.settings.to_auth(tag), ())
        } else {
            Authorization::Forbidden
        }
    }

    /// returns `User_Spec`s that match `invoking_user` and `hostname`
    ///
    /// it also distributes `Tag_Spec`s across the `Cmnd_Spec` list of each `User_Spec`
    ///
    /// the outer iterator are the `User_Spec`s; the inner iterator are the `Cmnd_Spec`s of
    /// said `User_Spec`s
    fn matching_user_specs<'a, User: UnixUser + PartialEq<User>>(
        &'a self,
        invoking_user: &'a User,
        hostname: &'a system::Hostname,
    ) -> impl Iterator<Item = impl Iterator<Item = MatchedCommand<'a>>> {
        let Self { rules, aliases, .. } = self;
        let user_aliases = get_aliases(&aliases.user, &match_user(invoking_user));
        let host_aliases = get_aliases(&aliases.host, &match_token(hostname));

        rules
            .iter()
            .filter_map(move |sudo| {
                find_item(&sudo.users, &match_user(invoking_user), &user_aliases)?;
                Some(&sudo.permissions)
            })
            .flatten()
            .filter_map(move |(hosts, runas_cmds)| {
                find_item(hosts, &match_token(hostname), &host_aliases)?;
                Some(distribute_tags(runas_cmds))
            })
    }

    pub fn matching_entries<'a, User: UnixUser + PartialEq<User>>(
        &'a self,
        invoking_user: &'a User,
        hostname: &'a system::Hostname,
    ) -> impl Iterator<Item = Entry<'a>> {
        let user_specs = self.matching_user_specs(invoking_user, hostname);

        user_specs.flat_map(|cmd_specs| group_cmd_specs_per_runas(cmd_specs, &self.aliases.cmnd))
    }

    pub(crate) fn visudo_editor_path<User: UnixUser + PartialEq<User>>(
        mut self,
        on_host: &system::Hostname,
        am_user: &User,
        target_user: &User,
    ) -> Option<PathBuf> {
        self.specify_host_user_runas(on_host, am_user, Some(target_user));

        select_editor(&self.settings, self.settings.env_editor())
    }
}

/// Retrieve the chosen editor from a settings object, filtering based on whether the
/// environment is trusted (sudoedit) or maybe less so (visudo)
fn select_editor(settings: &Settings, trusted_env: bool) -> Option<PathBuf> {
    let blessed_editors = settings.editor();

    let is_whitelisted = |path: &Path| -> bool {
        trusted_env || blessed_editors.split(':').any(|x| Path::new(x) == path)
    };

    // find editor in environment, if possible

    for key in ["SUDO_EDITOR", "VISUAL", "EDITOR"] {
        if let Some(editor) = std::env::var_os(key) {
            let editor = PathBuf::from(editor);

            let editor = if is_valid_executable(&editor) {
                editor
            } else if let Some(editor) = resolve_path(
                &editor,
                &std::env::var("PATH").unwrap_or(crate::sudo::PATH_DEFAULT.to_string()),
            ) {
                editor
            } else {
                continue;
            };

            if is_whitelisted(&editor) {
                return Some(editor);
            }
        }
    }

    // no acceptable editor found in environment, fallback on config

    for editor in blessed_editors.split(':') {
        let editor = PathBuf::from(editor);
        if is_valid_executable(&editor) {
            return Some(editor);
        }
    }

    None
}

// a `take_while` variant that does not consume the first non-matching item
fn peeking_take_while<'a, T>(
    iter: &'a mut std::iter::Peekable<impl Iterator<Item = T>>,
    pred: impl Fn(&T) -> bool + 'a,
) -> impl Iterator<Item = T> + 'a {
    std::iter::from_fn(move || iter.next_if(&pred))
}

fn group_cmd_specs_per_runas<'a>(
    cmnd_specs: impl Iterator<Item = (Option<&'a RunAs>, (Tag, &'a Spec<Command>))>,
    cmnd_aliases: &'a VecOrd<Def<Command>>,
) -> impl Iterator<Item = Entry<'a>> {
    // `distribute_tags` will have given every spec a reference to the "runas specification"
    // that applies to it. The output of sudo --list splits the CmndSpec list based on that:
    // every line only has a single "runas" specifier. So we need to combine them for that.
    //
    // But sudo --list also outputs lines that are from different lines in the sudoers file on
    // different lines in the output of sudo --list, so we cannot compare "by value". Luckily,
    // once a RunAs is parsed, it will have a unique identifier in the form of its address.
    let origin = |runas: Option<&RunAs>| runas.map(|r| r as *const _);

    let mut cmnd_specs = cmnd_specs.peekable();

    std::iter::from_fn(move || {
        if let Some(&(cur_runas, _)) = cmnd_specs.peek() {
            let specs = peeking_take_while(&mut cmnd_specs, |&(runas, _)| {
                origin(runas) == origin(cur_runas)
            });

            Some(Entry::new(
                cur_runas,
                specs.map(|x| x.1).collect(),
                cmnd_aliases,
            ))
        } else {
            None
        }
    })
}

fn read_sudoers<R: io::Read>(mut reader: R) -> io::Result<Vec<basic_parser::Parsed<Sudo>>> {
    // it's a bit frustrating that BufReader.chars() does not exist
    let mut buffer = String::new();
    reader.read_to_string(&mut buffer)?;

    use basic_parser::parse_lines;
    use char_stream::*;
    Ok(parse_lines(&mut CharStream::new(&buffer)))
}

fn open_sudoers(path: &Path) -> io::Result<Vec<basic_parser::Parsed<Sudo>>> {
    let source = audit::secure_open_sudoers(path, false)?;
    read_sudoers(source)
}

fn open_subsudoers(path: &Path) -> io::Result<Vec<basic_parser::Parsed<Sudo>>> {
    let source = audit::secure_open_sudoers(path, true)?;
    read_sudoers(source)
}

// note: trying to DRY using GAT's is tempting but doesn't make the code any shorter

#[derive(Default)]
struct AliasTable {
    user: VecOrd<Def<UserSpecifier>>,
    host: VecOrd<Def<Hostname>>,
    cmnd: VecOrd<Def<Command>>,
    runas: VecOrd<Def<UserSpecifier>>,
}

#[derive(Default)]
struct CustomiserTable {
    non_cmnd: Vec<Customiser<ConfigScope>>,
    cmnd: Vec<Customiser<SpecList<Command>>>,
}

/// A vector with a list defining the order in which it needs to be processed
struct VecOrd<T>(Vec<usize>, Vec<T>);

impl<T> Default for VecOrd<T> {
    fn default() -> Self {
        VecOrd(Vec::default(), Vec::default())
    }
}

impl<T> VecOrd<T> {
    fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + Clone {
        self.0.iter().map(|&i| &self.1[i])
    }
}

/// Check if the user `am_user` is allowed to run `cmdline` on machine `on_host` as the requested
/// user/group. Not that in the sudoers file, later permissions override earlier restrictions.
/// The `cmdline` argument should already be ready to essentially feed to an exec() call; or be
/// a special command like 'sudoedit'.
// This code is structure to allow easily reading the 'happy path'; i.e. as soon as something
// doesn't match, we escape using the '?' mechanism.
fn check_permission<User: UnixUser + PartialEq<User>, Group: UnixGroup>(
    sudoers: &Sudoers,
    am_user: &User,
    on_host: &system::Hostname,
    request: Request<User, Group>,
) -> Option<Tag> {
    let cmdline = (request.command, request.arguments);

    let aliases = &sudoers.aliases;
    let cmnd_aliases = get_aliases(&aliases.cmnd, &match_command(cmdline));
    let runas_user_aliases = get_aliases(&aliases.runas, &match_user(request.user));
    let runas_group_aliases = get_aliases(&aliases.runas, &match_group_alias(request.group));

    let matching_user_specs = sudoers.matching_user_specs(am_user, on_host).flatten();

    let allowed_commands = matching_user_specs.filter_map(|(runas, cmdspec)| {
        if let Some(RunAs { users, groups }) = runas {
            let stays_in_group = in_group(request.user, request.group);
            if request.user != am_user || (stays_in_group && !users.is_empty()) {
                find_item(users, &match_user(request.user), &runas_user_aliases)?
            }
            if !stays_in_group {
                find_item(groups, &match_group(request.group), &runas_group_aliases)?
            }
        } else if !(request.user.is_root() && in_group(request.user, request.group)) {
            None?;
        }

        Some(cmdspec)
    });

    find_item(allowed_commands, &match_command(cmdline), &cmnd_aliases)
}

/// Process a raw parsed AST bit of RunAs + Command specifications:
/// - RunAs specifications distribute over the commands that follow (until overridden)
/// - Tags accumulate over the entire line
fn distribute_tags(
    runas_cmds: &[(Option<RunAs>, CommandSpec)],
) -> impl Iterator<Item = (Option<&RunAs>, (Tag, &Spec<Command>))> {
    runas_cmds.iter().scan(
        (None, Default::default()),
        |(last_runas, tag), (runas, CommandSpec(mods, cmd))| {
            *last_runas = runas.as_ref().or(*last_runas);
            for f in mods {
                f(tag);
            }

            let this_tag = match cmd {
                Qualified::Allow(Meta::All) if tag.env != EnvironmentControl::Nosetenv => Tag {
                    // "ALL" has an implicit "SETENV" that doesn't distribute
                    env: EnvironmentControl::Setenv,
                    ..tag.clone()
                },
                _ => tag.clone(),
            };

            Some((*last_runas, (this_tag, cmd)))
        },
    )
}

/// A type to represent positive or negative association with an alias; i.e. if a key maps to true,
/// the alias affirms membership, if a key maps to false, the alias denies membership; if a key
/// isn't present membership is affirmed nor denied
type FoundAliases = HashMap<String, bool>;

/// Find an item matching a certain predicate in an collection (optionally attributed) list of
/// identifiers; identifiers can be directly identifying, wildcards, and can either be positive or
/// negative (i.e. preceeded by an even number of exclamation marks in the sudoers file)
fn find_item<'a, Predicate, Iter, T: 'a>(
    items: Iter,
    matches: &Predicate,
    aliases: &FoundAliases,
) -> Option<<Iter::Item as WithInfo>::Info>
where
    Predicate: Fn(&T) -> bool,
    Iter: IntoIterator,
    Iter::Item: WithInfo<Item = &'a Spec<T>>,
{
    let mut result = None;
    for item in items {
        let (judgement, who) = match item.as_inner() {
            Qualified::Forbid(x) => (false, x),
            Qualified::Allow(x) => (true, x),
        };
        let info = || item.into_info();
        match who {
            Meta::All => result = judgement.then(info),
            Meta::Only(ident) if matches(ident) => result = judgement.then(info),
            Meta::Alias(id) if aliases.contains_key(id) => {
                result = if aliases[id] {
                    judgement.then(info)
                } else {
                    // in this case, an explicit negation in the alias applies
                    (!judgement).then(info)
                }
            }
            _ => {}
        };
    }

    result
}

/// A interface to access optional "satellite data"
trait WithInfo {
    type Item;
    type Info;
    fn as_inner(&self) -> Self::Item;
    fn into_info(self) -> Self::Info;
}

/// A specific interface for `Spec<T>` --- we can't make a generic one;
/// A `Spec<T>` does not contain any additional information.
impl<'a, T> WithInfo for &'a Spec<T> {
    type Item = &'a Spec<T>;
    type Info = ();
    fn as_inner(&self) -> &'a Spec<T> {
        self
    }
    fn into_info(self) {}
}

/// A commandspec can be "tagged"
impl<'a> WithInfo for (Tag, &'a Spec<Command>) {
    type Item = &'a Spec<Command>;
    type Info = Tag;
    fn as_inner(&self) -> &'a Spec<Command> {
        self.1
    }
    fn into_info(self) -> Tag {
        self.0
    }
}

/// Now follow a collection of functions used as closures for `find_item`
fn match_user(user: &impl UnixUser) -> impl Fn(&UserSpecifier) -> bool + '_ {
    move |spec| match spec {
        UserSpecifier::User(id) => match_identifier(user, id),
        UserSpecifier::Group(Identifier::Name(name)) => user.in_group_by_name(name.as_cstr()),
        UserSpecifier::Group(Identifier::ID(num)) => user.in_group_by_gid(GroupId::new(*num)),
        // nonunix-groups, netgroups, etc. are not implemented
        UserSpecifier::NonunixGroup(group) => {
            match group {
                Identifier::Name(name) => auth_warn!("warning: non-unix group {name} was ignored"),
                Identifier::ID(num) => auth_warn!("warning: non-unix group #{num} was ignored"),
            }

            false
        }
    }
}

fn in_group(user: &impl UnixUser, group: &impl UnixGroup) -> bool {
    user.in_group_by_gid(group.as_gid())
}

fn match_group(group: &impl UnixGroup) -> impl Fn(&Identifier) -> bool + '_ {
    move |id| match id {
        Identifier::ID(num) => group.as_gid() == GroupId::new(*num),
        Identifier::Name(name) => group.try_as_name().is_some_and(|s| name == s),
    }
}

fn match_group_alias(group: &impl UnixGroup) -> impl Fn(&UserSpecifier) -> bool + '_ {
    move |spec| match spec {
        UserSpecifier::User(ident) => match_group(group)(ident),
        /* the parser does not allow this, but can happen due to Runas_Alias,
         * see https://github.com/trifectatechfoundation/sudo-rs/issues/13 */
        _ => {
            auth_warn!("warning: ignoring %group syntax in runas_alias for checking sudo -g");
            false
        }
    }
}

fn match_token<T: basic_parser::Token + std::ops::Deref<Target = String>>(
    text: &str,
) -> impl Fn(&T) -> bool + '_ {
    move |token| token.as_str() == text
}

fn match_command<'a>((cmd, args): (&'a Path, &'a [OsString])) -> impl Fn(&Command) -> bool + 'a {
    let opts = glob::MatchOptions {
        require_literal_separator: true,
        ..glob::MatchOptions::new()
    };
    move |(cmdpat, argpat)| {
        cmdpat.matches_path_with(cmd, opts)
            && match argpat {
                Args::Prefix(vec) => args.starts_with(vec),
                Args::Exact(vec) => args == vec.as_ref(),
            }
    }
}

/// Find all the aliases that a object is a member of; this requires [sanitize_alias_table] to have run first;
/// I.e. this function should not be "pub".
fn get_aliases<Predicate, T>(table: &VecOrd<Def<T>>, pred: &Predicate) -> FoundAliases
where
    Predicate: Fn(&T) -> bool,
{
    use std::iter::once;
    let all = Qualified::Allow(Meta::All);

    let mut set = HashMap::new();
    for Def(id, list) in table.iter() {
        if find_item(list, &pred, &set).is_some() {
            set.insert(id.clone(), true);
        } else if find_item(once(&all).chain(list), &pred, &set).is_none() {
            // the item wasn't found even if we prepend ALL to the list of definitions; that means
            // it is explicitly excluded by the alias definition.
            set.insert(id.clone(), false);
        }
    }

    set
}

/// Code to map an ast::Identifier to the UnixUser trait
fn match_identifier(user: &impl UnixUser, ident: &ast::Identifier) -> bool {
    match ident {
        Identifier::Name(name) => user.has_name(name),
        Identifier::ID(num) => user.has_uid(UserId::new(*num)),
    }
}

/// Process a sudoers-parsing file into a workable AST
fn analyze(
    path: &Path,
    sudoers: impl IntoIterator<Item = basic_parser::Parsed<Sudo>>,
) -> (Sudoers, Vec<Error>) {
    use Directive::*;

    let mut result: Sudoers = Default::default();

    fn resolve_relative(base: &Path, path: impl AsRef<Path>) -> PathBuf {
        if path.as_ref().is_relative() {
            // there should always be a parent since we start with /etc/sudoers, and make every other path
            // absolute based on previous inputs; not having a parent is therefore a serious bug
            base.parent()
                .expect("invalid hardcoded path in sudo-rs")
                .join(path)
        } else {
            path.as_ref().into()
        }
    }

    fn include(
        cfg: &mut Sudoers,
        parent: &Path,
        span: Span,
        path: &Path,
        diagnostics: &mut Vec<Error>,
        count: &mut u8,
    ) {
        if *count >= INCLUDE_LIMIT {
            diagnostics.push(Error {
                source: Some(parent.to_owned()),
                location: Some(span),
                message: format!("include file limit reached opening '{}'", path.display()),
            })
        // FIXME: this will cause an error in `visudo` if we open a non-privileged sudoers file
        // that includes another non-privileged sudoer files.
        } else {
            match open_subsudoers(path) {
                Ok(subsudoer) => {
                    *count += 1;
                    process(cfg, path, subsudoer, diagnostics, count)
                }
                Err(e) => {
                    let message = if e.kind() == io::ErrorKind::NotFound {
                        // improve the error message in this case
                        format!("cannot open sudoers file '{}'", path.display())
                    } else {
                        e.to_string()
                    };

                    diagnostics.push(Error {
                        source: Some(parent.to_owned()),
                        location: Some(span),
                        message,
                    })
                }
            }
        }
    }

    fn process(
        cfg: &mut Sudoers,
        cur_path: &Path,
        sudoers: impl IntoIterator<Item = basic_parser::Parsed<Sudo>>,
        diagnostics: &mut Vec<Error>,
        safety_count: &mut u8,
    ) {
        for item in sudoers {
            match item {
                Ok(line) => match line {
                    Sudo::LineComment => {}

                    Sudo::Spec(permission) => {
                        diagnostics.extend(get_ignored_tags(&permission).map(|span| Error {
                            source: Some(cur_path.to_owned()),
                            location: Some(span),
                            message: "this tag is ignored by sudo-rs".to_string(),
                        }));
                        cfg.rules.push(permission);
                    }

                    Sudo::Decl(HostAlias(mut def)) => cfg.aliases.host.1.append(&mut def),
                    Sudo::Decl(UserAlias(mut def)) => cfg.aliases.user.1.append(&mut def),
                    Sudo::Decl(RunasAlias(mut def)) => cfg.aliases.runas.1.append(&mut def),
                    Sudo::Decl(CmndAlias(mut def)) => cfg.aliases.cmnd.1.append(&mut def),

                    Sudo::Decl(Defaults(params, scope)) => {
                        if let ConfigScope::Command(specs) = scope {
                            cfg.customisers.cmnd.push((
                                specs
                                    .into_iter()
                                    .map(|spec| {
                                        spec.map(|simple_command| {
                                            (simple_command, Args::Prefix(Box::default()))
                                        })
                                    })
                                    .collect(),
                                params,
                            ));
                        } else {
                            cfg.customisers.non_cmnd.push((scope, params));
                        }
                    }

                    Sudo::Include(path, span) => include(
                        cfg,
                        cur_path,
                        span,
                        &resolve_relative(cur_path, path),
                        diagnostics,
                        safety_count,
                    ),

                    Sudo::IncludeDir(path, span) => {
                        if path.contains("%h") {
                            diagnostics.push(Error {
                                source: Some(cur_path.to_owned()),
                                location: Some(span),
                                message: format!(
                                    "cannot open sudoers file {path}: \
                                     percent escape %h in includedir is unsupported"
                                ),
                            });
                            continue;
                        }

                        let path = resolve_relative(cur_path, path);
                        let Ok(files) = std::fs::read_dir(&path) else {
                            diagnostics.push(Error {
                                source: Some(cur_path.to_owned()),
                                location: Some(span),
                                message: format!("cannot open sudoers file {}", path.display()),
                            });
                            continue;
                        };
                        let mut safe_files = files
                            .filter_map(|direntry| {
                                let path = direntry.ok()?.path();
                                let text = path.file_name()?.to_str()?;
                                if text.ends_with('~') || text.contains('.') {
                                    None
                                } else {
                                    Some(path)
                                }
                            })
                            .collect::<Vec<_>>();
                        safe_files.sort();
                        for file in safe_files {
                            include(
                                cfg,
                                cur_path,
                                span,
                                file.as_ref(),
                                diagnostics,
                                safety_count,
                            )
                        }
                    }
                },

                Err(basic_parser::Status::Fatal(pos, message)) => diagnostics.push(Error {
                    source: Some(cur_path.to_owned()),
                    location: Some(pos),
                    message,
                }),
                Err(_) => panic!("internal parser error"),
            }
        }
    }

    fn get_ignored_tags(
        PermissionSpec { permissions, .. }: &PermissionSpec,
    ) -> impl Iterator<Item = Span> + '_ {
        permissions
            .iter()
            .flat_map(|(_host, runas_cmds)| runas_cmds)
            .flat_map(|(_runas, CommandSpec(tags, _cmd))| tags)
            .flat_map(|modifier| {
                let mut tag = Tag::default();
                modifier(&mut tag);
                tag.ignored
            })
    }

    let mut diagnostics = vec![];
    process(&mut result, path, sudoers, &mut diagnostics, &mut 0);

    let alias = &mut result.aliases;
    alias.user.0 = sanitize_alias_table(&alias.user.1, &mut diagnostics);
    alias.host.0 = sanitize_alias_table(&alias.host.1, &mut diagnostics);
    alias.cmnd.0 = sanitize_alias_table(&alias.cmnd.1, &mut diagnostics);
    alias.runas.0 = sanitize_alias_table(&alias.runas.1, &mut diagnostics);

    (result, diagnostics)
}

/// Alias definition inin a Sudoers file can come in any order; and aliases can refer to other aliases, etc.
/// It is much easier if they are presented in a "definitional order" (i.e. aliases that use other aliases occur later)
/// At the same time, this is a good place to detect problems in the aliases, such as unknown aliases and cycles.
fn sanitize_alias_table<T>(table: &Vec<Def<T>>, diagnostics: &mut Vec<Error>) -> Vec<usize> {
    fn remqualify<U>(item: &Qualified<U>) -> &U {
        match item {
            Qualified::Allow(x) => x,
            Qualified::Forbid(x) => x,
        }
    }

    // perform a topological sort (hattip david@tweedegolf.com) to produce a derangement
    struct Visitor<'a, T> {
        seen: HashSet<usize>,
        table: &'a Vec<Def<T>>,
        order: Vec<usize>,
        diagnostics: &'a mut Vec<Error>,
    }

    impl<T> Visitor<'_, T> {
        fn complain(&mut self, text: String) {
            self.diagnostics.push(Error {
                source: None,
                location: None,
                message: text,
            })
        }

        fn visit(&mut self, pos: usize) {
            if self.seen.insert(pos) {
                let Def(_, members) = &self.table[pos];
                for elem in members {
                    let Meta::Alias(name) = remqualify(elem) else {
                        continue;
                    };
                    let Some(dependency) = self.table.iter().position(|Def(id, _)| id == name)
                    else {
                        self.complain(format!("undefined alias: '{name}'"));
                        continue;
                    };
                    self.visit(dependency);
                }
                self.order.push(pos);
            } else if !self.order.contains(&pos) {
                let Def(id, _) = &self.table[pos];
                self.complain(format!("recursive alias: '{id}'"));
            }
        }
    }

    let mut visitor = Visitor {
        seen: HashSet::new(),
        table,
        order: Vec::with_capacity(table.len()),
        diagnostics,
    };

    let mut dupe = HashSet::new();
    for (i, Def(name, _)) in table.iter().enumerate() {
        if !dupe.insert(name) {
            visitor.complain(format!("multiple occurrences of '{name}'"));
        } else {
            visitor.visit(i);
        }
    }

    visitor.order
}

#[cfg(test)]
mod test;
