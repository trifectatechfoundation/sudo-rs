//! Code that checks (and in the future: lists) permissions in the sudoers file

mod ast;
mod ast_names;
mod basic_parser;
mod char_stream;
mod tokens;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use ast::*;
use sudo_system::interface::{UnixGroup, UnixUser};
use tokens::*;

/// How many nested include files do we allow?
const INCLUDE_LIMIT: u8 = 128;

/// Export some necessary symbols from modules
pub use ast::Tag;
pub struct Error(pub Option<basic_parser::Position>, pub String);

#[derive(Default)]
pub struct Sudoers {
    rules: Vec<PermissionSpec>,
    aliases: AliasTable,
    pub settings: Settings,
}

pub struct Request<'a, User: UnixUser, Group: UnixGroup> {
    pub user: &'a User,
    pub group: &'a Group,
}

/// This function takes a file argument for a sudoers file and processes it.

pub fn compile(path: impl AsRef<Path>) -> Result<(Sudoers, Vec<Error>), std::io::Error> {
    let sudoers = read_sudoers(path.as_ref())?;
    Ok(analyze(sudoers))
}

fn read_sudoers(path: &Path) -> Result<Vec<basic_parser::Parsed<Sudo>>, std::io::Error> {
    use std::fs::File;
    use std::io::Read;
    let mut source = File::open(path)?;

    // it's a bit frustrating that BufReader.chars() does not exist
    let mut buffer = String::new();
    source.read_to_string(&mut buffer)?;

    use basic_parser::parse_lines;
    use char_stream::*;
    Ok(parse_lines(&mut PeekableWithPos::new(buffer.chars())))
}

#[derive(Default)]
pub(crate) struct AliasTable {
    user: VecOrd<Def<UserSpecifier>>,
    host: VecOrd<Def<Hostname>>,
    cmnd: VecOrd<Def<Command>>,
    runas: VecOrd<Def<UserSpecifier>>,
}

/// A vector with a list defining the order in which it needs to be processed

type VecOrd<T> = (Vec<usize>, Vec<T>);

fn elems<T>(vec: &VecOrd<T>) -> impl Iterator<Item = &T> {
    vec.0.iter().map(|&i| &vec.1[i])
}

/// Check if the user `am_user` is allowed to run `cmdline` on machine `on_host` as the requested
/// user/group. Not that in the sudoers file, later permissions override earlier restrictions.
/// The `cmdline` argument should already be ready to essentially feed to an exec() call; or be
/// a special command like 'sudoedit'.

// This code is structure to allow easily reading the 'happy path'; i.e. as soon as something
// doesn't match, we escape using the '?' mechanism.
pub fn check_permission<User: UnixUser + PartialEq<User>, Group: UnixGroup>(
    Sudoers {
        rules,
        aliases,
        settings: _,
    }: &Sudoers,
    am_user: &User,
    request: Request<User, Group>,
    on_host: &str,
    cmdline: &str,
) -> Option<Vec<Tag>> {
    let user_aliases = get_aliases(&aliases.user, &match_user(am_user));
    let host_aliases = get_aliases(&aliases.host, &match_token(on_host));
    let cmnd_aliases = get_aliases(&aliases.cmnd, &match_command(cmdline));
    let runas_user_aliases = get_aliases(&aliases.runas, &match_user(request.user));
    let runas_group_aliases = get_aliases(&aliases.runas, &match_group_alias(request.group));

    let mut sha2_eq = check_all_sha2(cmdline);

    let allowed_commands = rules
        .iter()
        .filter_map(|sudo| {
            find_item(&sudo.users, &match_user(am_user), &user_aliases)?;

            let matching_rules = sudo
                .permissions
                .iter()
                .filter_map(|(hosts, runas, cmds)| {
                    find_item(hosts, &match_token(on_host), &host_aliases)?;

                    if let Some(RunAs { users, groups }) = runas {
                        if !users.is_empty() || request.user != am_user {
                            *find_item(users, &match_user(request.user), &runas_user_aliases)?
                        }
                        if !in_group(request.user, request.group) {
                            *find_item(groups, &match_group(request.group), &runas_group_aliases)?
                        }
                    } else if !(request.user.is_root() && in_group(request.user, request.group)) {
                        None?;
                    }

                    Some(cmds)
                })
                .flatten();

            Some(matching_rules.collect::<Vec<_>>())
        })
        .flatten()
        .filter(|CommandSpec(_, _, Sha2(hex))| hex.is_empty() || sha2_eq(hex));

    find_item(allowed_commands, &match_command(cmdline), &cmnd_aliases).cloned()
}

/// Find an item matching a certain predicate in an collection (optionally attributed) list of
/// identifiers; identifiers can be directly identifying, wildcards, and can either be positive or
/// negative (i.e. preceeded by an even number of exclamation marks in the sudoers file)

fn find_item<'a, Predicate, T, Permit: Tagged<T> + 'a>(
    items: impl IntoIterator<Item = &'a Permit>,
    matches: &Predicate,
    aliases: &HashSet<String>,
) -> Option<&'a Permit::Flags>
where
    Predicate: Fn(&T) -> bool,
{
    let mut result = None;
    for item in items {
        let (judgement, who) = match item.into() {
            Qualified::Forbid(x) => (None, x),
            Qualified::Allow(x) => (Some(item.to_info()), x),
        };
        match who {
            Meta::All => result = judgement,
            Meta::Only(ident) if matches(ident) => result = judgement,
            Meta::Alias(id) if aliases.contains(id) => result = judgement,
            _ => {}
        };
    }
    result
}

fn match_user(user: &impl UnixUser) -> impl Fn(&UserSpecifier) -> bool + '_ {
    move |spec| match spec {
        UserSpecifier::User(id) => match_identifier(user, id),
        UserSpecifier::Group(Identifier::Name(name)) => user.in_group_by_name(name),
        UserSpecifier::Group(Identifier::ID(num)) => user.in_group_by_gid(*num),
        _ => todo!(), // nonunix-groups, netgroups, etc.
    }
}

//TODO: in real life, just checking the gid should suffice; for testability, we check the name first; THIS MUST BE REMOVED
fn in_group(user: &impl UnixUser, group: &impl UnixGroup) -> bool {
    if cfg!(test) {
        group
            .try_as_name()
            .as_ref()
            .map_or(user.in_group_by_gid(group.as_gid()), |name| {
                user.in_group_by_name(name)
            })
    } else {
        user.in_group_by_gid(group.as_gid())
    }
}

fn match_group(group: &impl UnixGroup) -> impl Fn(&Identifier) -> bool + '_ {
    move |id| match id {
        Identifier::ID(num) => group.as_gid() == *num,
        Identifier::Name(name) => group.try_as_name().map_or(false, |s| s == name),
    }
}

fn match_group_alias(group: &impl UnixGroup) -> impl Fn(&UserSpecifier) -> bool + '_ {
    move |spec| match spec {
        UserSpecifier::User(ident) => match_group(group)(ident),
        /* the parser does not allow this, but can happen due to Runas_Alias,
         * see https://github.com/memorysafety/sudo-rs/issues/13 */
        _ => {
            eprintln!("warning: ignoring %group syntax in runas_alias for checking sudo -g");
            false
        }
    }
}

fn match_token<T: basic_parser::Token + std::ops::Deref<Target = String>>(
    text: &str,
) -> (impl Fn(&T) -> bool + '_) {
    move |token| token.as_str() == text
}

fn match_command(text: &str) -> (impl Fn(&Command) -> bool + '_) {
    let text = split_args(text);
    let (cmd, args) = (text[0], text[1..].join(" "));
    move |(cmdpat, argpat)| cmdpat.matches(cmd) && argpat.matches(&args)
}

/// Find all the aliases that a object is a member of; this requires [sanitize_alias_table] to have run first;
/// I.e. this function should not be "pub".

fn get_aliases<Predicate, T>(table: &VecOrd<Def<T>>, pred: &Predicate) -> HashSet<String>
where
    Predicate: Fn(&T) -> bool,
{
    let mut set = HashSet::new();
    for Def(id, list) in elems(table) {
        if find_item(list, &pred, &set).is_some() {
            set.insert(id.clone());
        }
    }

    set
}

/// Code to map an ast::Identifier to the UnixUser trait

fn match_identifier(user: &impl UnixUser, ident: &ast::Identifier) -> bool {
    match ident {
        Identifier::Name(name) => user.has_name(name),
        Identifier::ID(num) => user.has_uid(*num),
    }
}

//TODO: don't derive Default, but implement it (based on what the actual defaults are)
#[derive(Debug, Default)]
pub struct Settings {
    pub flags: HashSet<String>,
    pub str_value: HashMap<String, String>,
    pub list: HashMap<String, HashSet<String>>,
}

/// Process a sudoers-parsing file into a workable AST
fn analyze(sudoers: impl IntoIterator<Item = basic_parser::Parsed<Sudo>>) -> (Sudoers, Vec<Error>) {
    use DefaultValue::*;
    use Directive::*;

    let mut result: Sudoers = Default::default();

    impl Sudoers {
        fn include(&mut self, path: &Path, diagnostics: &mut Vec<Error>, count: &mut u8) {
            if *count >= INCLUDE_LIMIT {
                diagnostics.push(Error(
                    None,
                    format!("include file limit reached opening `{}'", path.display()),
                ))
            } else if let Ok(subsudoer) = read_sudoers(path) {
                *count += 1;
                self.process(subsudoer, diagnostics, count)
            } else {
                diagnostics.push(Error(
                    None,
                    format!("cannot open sudoers file `{}'", path.display()),
                ))
            }
        }

        fn process(
            &mut self,
            sudoers: impl IntoIterator<Item = basic_parser::Parsed<Sudo>>,
            diagnostics: &mut Vec<Error>,
            safety_count: &mut u8,
        ) {
            for item in sudoers {
                match item {
                    Ok(line) => match line {
                        Sudo::LineComment => {}

                        Sudo::Spec(permission) => self.rules.push(permission),

                        Sudo::Decl(UserAlias(def)) => self.aliases.user.1.push(def),
                        Sudo::Decl(HostAlias(def)) => self.aliases.host.1.push(def),
                        Sudo::Decl(CmndAlias(def)) => self.aliases.cmnd.1.push(def),
                        Sudo::Decl(RunasAlias(def)) => self.aliases.runas.1.push(def),

                        Sudo::Decl(Defaults(name, Flag(value))) => {
                            if value {
                                self.settings.flags.insert(name);
                            } else {
                                self.settings.flags.remove(&name);
                            }
                        }
                        Sudo::Decl(Defaults(name, Text(value))) => {
                            self.settings.str_value.insert(name, value);
                        }

                        Sudo::Decl(Defaults(name, List(mode, values))) => {
                            let slot: &mut _ = self.settings.list.entry(name).or_default();
                            match mode {
                                Mode::Set => *slot = values.into_iter().collect(),
                                Mode::Add => slot.extend(values),
                                Mode::Del => {
                                    for key in values {
                                        slot.remove(&key);
                                    }
                                }
                            }
                        }

                        Sudo::Include(path) => {
                            self.include(path.as_ref(), diagnostics, safety_count)
                        }

                        Sudo::IncludeDir(path) => {
                            let Ok(files) = std::fs::read_dir(&path) else {
                                diagnostics.push(Error(None, format!("cannot open sudoers file {path}")));
                                continue;
                            };
                            let mut safe_files = files
                                .filter_map(|direntry| {
                                    let path = direntry.ok()?.path();
                                    let text = path.to_str()?;
                                    if text.ends_with('~') || text.contains('.') {
                                        None
                                    } else {
                                        Some(path)
                                    }
                                })
                                .collect::<Vec<_>>();
                            safe_files.sort();
                            for file in safe_files {
                                self.include(file.as_ref(), diagnostics, safety_count)
                            }
                        }
                    },

                    Err(basic_parser::Status::Fatal(pos, error)) => {
                        diagnostics.push(Error(Some(pos), error))
                    }
                    Err(_) => panic!("internal parser error"),
                }
            }
        }
    }

    let mut diagnostics = vec![];
    result.process(sudoers, &mut diagnostics, &mut 0);

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
            self.diagnostics.push(Error(None, text))
        }

        fn visit(&mut self, pos: usize) {
            if self.seen.insert(pos) {
                let Def(_, members) = &self.table[pos];
                for elem in members {
                    let Meta::Alias(name) = remqualify(elem) else { break };
                    let Some(dependency) = self.table.iter().position(|Def(id,_)| id==name) else {
                        self.complain(format!("undefined alias: `{name}'"));
                        continue;
                    };
                    self.visit(dependency);
                }
                self.order.push(pos);
            } else if !self.order.contains(&pos) {
                let Def(id, _) = &self.table[pos];
                self.complain(format!("recursive alias: `{id}'"));
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
            visitor.complain(format!("multiple occurences of `{name}'"));
        } else {
            visitor.visit(i);
        }
    }

    visitor.order
}

mod compute_hash;

fn check_all_sha2(cmdline: &str) -> impl FnMut(&Box<[u8]>) -> bool + '_ {
    use compute_hash::sha2;

    let mut memo = std::collections::HashMap::new(); // pun not intended

    move |bytes| {
        let bits = 8 * bytes.len() as u16;
        memo.entry(bits).or_insert_with(|| sha2(bits, cmdline)) == bytes
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast;
    use basic_parser::{parse_eval, parse_lines, parse_string};
    use std::iter;

    #[derive(PartialEq)]
    struct Named(&'static str);

    impl UnixUser for Named {
        fn has_name(&self, name: &str) -> bool {
            self.0 == name
        }

        fn in_group_by_name(&self, name: &str) -> bool {
            self.has_name(name)
        }

        fn is_root(&self) -> bool {
            self.0 == "root"
        }
    }

    impl UnixGroup for Named {
        fn as_gid(&self) -> sudo_system::interface::GroupId {
            if self.0 == "root" {
                0
            } else {
                self.0.chars().fold(0, |x, y| (x * 97 + y as u32) % 1361)
            }
        }
        fn try_as_name(&self) -> Option<&str> {
            Some(&self.0)
        }
    }

    macro_rules! request {
        ($user:ident, $group:ident) => {
            Request {
                user: &Named(stringify!($user)),
                group: &Named(stringify!($group)),
            }
        };
    }

    macro_rules! sudoer {
        ($h:expr $(,$e:expr)*) => {
	    parse_lines(&mut
		(
		    iter::once($h)
		    $(
			.chain(iter::once($e))
		    )*
		)
		.map(|s|s.chars().chain(iter::once('\n')))
		.flatten()
		.peekable()
	    )
	    .into_iter()
	    .map(|x| Ok::<_,basic_parser::Status>(x.unwrap()))
        }
    }

    // alternative to parse_eval, but goes through sudoer! directly
    fn parse_line(s: &str) -> Sudo {
        sudoer![s].next().unwrap().unwrap()
    }

    #[test]
    fn ambiguous_spec() {
        let Sudo::Spec(_) = parse_eval::<ast::Sudo>("marc, User_Alias ALL = ALL") else { todo!() };
    }

    #[test]
    fn digest_spec() {
        let CommandSpec(_, _, digest) = parse_eval(
            "NOPASSWD: sha224: c12053ca894181bc137b940b06b2e2459e9aa7b46d2d317777f34236 /bin/ls",
        );
        let Sha2(vec) = digest;
        assert_eq!(
            *vec,
            [
                0xc1, 0x20, 0x53, 0xca, 0x89, 0x41, 0x81, 0xbc, 0x13, 0x7b, 0x94, 0x0b, 0x06, 0xb2,
                0xe2, 0x45, 0x9e, 0x9a, 0xa7, 0xb4, 0x6d, 0x2d, 0x31, 0x77, 0x77, 0xf3, 0x42, 0x36,
            ]
        )
    }

    #[test]
    #[should_panic]
    fn digest_spec_fail1() {
        // the hash length is incorrect
        parse_eval::<CommandSpec>(
            "NOPASSWD: sha224: c12053ca894181bc137b940b06b2e2459e9aa7b46d2d317777f342 /bin/ls",
        );
    }

    #[test]
    #[should_panic]
    fn digest_spec_fail2() {
        // the hash length has an odd length
        parse_eval::<CommandSpec>(
            "NOPASSWD: sha224: c12053ca894181bc137b940b06b2e2459e9aa7b46d2d317777f3421 /bin/ls",
        );
    }

    #[test]
    #[should_panic]
    fn digest_spec_fail3() {
        // the hash length has an invalid char
        parse_eval::<CommandSpec>(
            "NOPASSWD: sha224: c12053ca894181bc137b940b06b2e2459e9aa7b46d2d317777g34236 /bin/ls",
        );
    }

    #[test]
    fn permission_test() {
        let root = || Request::<Named, Named> {
            user: &Named("root"),
            group: &Named("root"),
        };

        macro_rules! FAIL {
            ([$($sudo:expr),*], $user:expr => $req:expr, $server:expr; $command:expr) => {
                let (Sudoers { rules,aliases,settings }, _) = analyze(sudoer![$($sudo),*]);
                assert_eq!(check_permission(&Sudoers { rules, aliases, settings }, &Named($user), $req, $server, $command), None);
            }
        }

        macro_rules! pass {
            ([$($sudo:expr),*], $user:expr => $req:expr, $server:expr; $command:expr $(=> [$($list:expr),*])?) => {
                let (Sudoers { rules,aliases,settings }, _) = analyze(sudoer![$($sudo),*]);
                let result = check_permission(&Sudoers { rules, aliases, settings }, &Named($user), $req, $server, $command);
                $(assert_eq!(result, Some(vec![$($list),*]));)?
                assert!(!result.is_none());
            }
        }
        macro_rules! SYNTAX {
            ([$sudo:expr]) => {
                assert!(parse_string::<Sudo>($sudo).is_err())
            };
        }

        use crate::ast::Tag::*;

        SYNTAX!(["ALL ALL = (;) ALL"]);
        FAIL!(["user ALL=(ALL:ALL) ALL"], "nobody"    => root(), "server"; "/bin/hello");
        pass!(["user ALL=(ALL:ALL) ALL"], "user"      => root(), "server"; "/bin/hello");
        pass!(["user ALL=(ALL:ALL) /bin/foo"], "user" => root(), "server"; "/bin/foo");
        FAIL!(["user ALL=(ALL:ALL) /bin/foo"], "user" => root(), "server"; "/bin/hello");
        pass!(["user ALL=(ALL:ALL) /bin/foo, NOPASSWD: /bin/bar"], "user" => root(), "server"; "/bin/foo");
        pass!(["user ALL=(ALL:ALL) /bin/foo, NOPASSWD: /bin/bar"], "user" => root(), "server"; "/bin/bar" => [NoPasswd]);

        pass!(["user ALL=/bin/e##o"], "user" => root(), "vm"; "/bin/e");
        SYNTAX!(["ALL ALL=(ALL) /bin/\n/echo"]);

        pass!(["user server=(ALL:ALL) ALL"], "user" => root(), "server"; "/bin/hello");
        FAIL!(["user laptop=(ALL:ALL) ALL"], "user" => root(), "server"; "/bin/hello");

        pass!(["user ALL=!/bin/hello", "user ALL=/bin/hello"], "user" => root(), "server"; "/bin/hello");
        FAIL!(["user ALL=/bin/hello", "user ALL=!/bin/hello"], "user" => root(), "server"; "/bin/hello");

        for alias in [
            "User_Alias GROUP=user1, user2",
            "User_Alias GROUP=ALL,!user3",
        ] {
            pass!([alias,"GROUP ALL=/bin/hello"], "user1" => root(), "server"; "/bin/hello");
            pass!([alias,"GROUP ALL=/bin/hello"], "user2" => root(), "server"; "/bin/hello");
            FAIL!([alias,"GROUP ALL=/bin/hello"], "user3" => root(), "server"; "/bin/hello");
        }
        pass!(["user ALL=/bin/hello arg"], "user" => root(), "server"; "/bin/hello arg");
        pass!(["user ALL=/bin/hello  arg"], "user" => root(), "server"; "/bin/hello arg");
        pass!(["user ALL=/bin/hello arg"], "user" => root(), "server"; "/bin/hello  arg");
        FAIL!(["user ALL=/bin/hello arg"], "user" => root(), "server"; "/bin/hello boo");
        pass!(["user ALL=/bin/hello a*g"], "user" => root(), "server"; "/bin/hello  aaaarg");
        FAIL!(["user ALL=/bin/hello a*g"], "user" => root(), "server"; "/bin/hello boo");
        pass!(["user ALL=/bin/hello"], "user" => root(), "server"; "/bin/hello boo");
        FAIL!(["user ALL=/bin/hello \"\""], "user" => root(), "server"; "/bin/hello boo");
        pass!(["user ALL=/bin/hello \"\""], "user" => root(), "server"; "/bin/hello");
        pass!(["user ALL=/bin/hel*"], "user" => root(), "server"; "/bin/hello");
        pass!(["user ALL=/bin/hel*"], "user" => root(), "server"; "/bin/help");
        pass!(["user ALL=/bin/hel*"], "user" => root(), "server"; "/bin/help me");
        pass!(["user ALL=/bin/hel* *"], "user" => root(), "server"; "/bin/help");
        FAIL!(["user ALL=/bin/hel* me"], "user" => root(), "server"; "/bin/help");
        pass!(["user ALL=/bin/hel* me"], "user" => root(), "server"; "/bin/help me");
        FAIL!(["user ALL=/bin/hel* me"], "user" => root(), "server"; "/bin/help me please");

        SYNTAX!(["User_Alias, marc ALL = ALL"]);

        pass!(["User_Alias FULLTIME=ALL,!marc","FULLTIME ALL=ALL"], "user" => root(), "server"; "/bin/bash");
        FAIL!(["User_Alias FULLTIME=ALL,!marc","FULLTIME ALL=ALL"], "marc" => root(), "server"; "/bin/bash");
        FAIL!(["User_Alias FULLTIME=ALL,!marc","ALL,!FULLTIME ALL=ALL"], "user" => root(), "server"; "/bin/bash");
        pass!(["User_Alias FULLTIME=ALL,!marc","ALL,!FULLTIME ALL=ALL"], "marc" => root(), "server"; "/bin/bash");
        pass!(["Host_Alias MACHINE=laptop,server","user MACHINE=ALL"], "user" => root(), "server"; "/bin/bash");
        pass!(["Host_Alias MACHINE=laptop,server","user MACHINE=ALL"], "user" => root(), "laptop"; "/bin/bash");
        FAIL!(["Host_Alias MACHINE=laptop,server","user MACHINE=ALL"], "user" => root(), "desktop"; "/bin/bash");
        pass!(["Cmnd_Alias WHAT=/bin/dd, /bin/rm","user ALL=WHAT"], "user" => root(), "server"; "/bin/rm");
        pass!(["Cmd_Alias WHAT=/bin/dd,/bin/rm","user ALL=WHAT"], "user" => root(), "laptop"; "/bin/dd");
        FAIL!(["Cmnd_Alias WHAT=/bin/dd,/bin/rm","user ALL=WHAT"], "user" => root(), "desktop"; "/bin/bash");

        pass!(["User_Alias A=B","User_Alias B=user","A ALL=ALL"], "user" => root(), "vm"; "/bin/ls");
        pass!(["Host_Alias A=B","Host_Alias B=vm","ALL A=ALL"], "user" => root(), "vm"; "/bin/ls");
        pass!(["Cmnd_Alias A=B","Cmnd_Alias B=/bin/ls","ALL ALL=A"], "user" => root(), "vm"; "/bin/ls");

        FAIL!(["Runas_Alias TIME=%wheel,sudo","user ALL=() ALL"], "user" => request!{ sudo, sudo }, "vm"; "/bin/ls");
        pass!(["Runas_Alias TIME=%wheel,sudo","user ALL=(TIME) ALL"], "user" => request! { sudo, sudo }, "vm"; "/bin/ls");
        FAIL!(["Runas_Alias TIME=%wheel,sudo","user ALL=(:TIME) ALL"], "user" => request! { sudo, sudo }, "vm"; "/bin/ls");
        pass!(["Runas_Alias TIME=%wheel,sudo","user ALL=(:TIME) ALL"], "user" => request! { user, sudo }, "vm"; "/bin/ls");
        pass!(["Runas_Alias TIME=%wheel,sudo","user ALL=(TIME) ALL"], "user" => request! { wheel, wheel }, "vm"; "/bin/ls");

        pass!(["Runas_Alias \\"," TIME=%wheel\\",",sudo # hallo","user ALL\\","=(TIME) ALL"], "user" => request! { wheel, wheel }, "vm"; "/bin/ls");
    }

    #[test]
    #[should_panic]
    fn invalid_directive() {
        parse_eval::<ast::Sudo>("User_Alias, user Alias = user1, user2");
    }

    use std::ops::Neg;
    use Qualified::*;
    impl<T> Neg for Qualified<T> {
        type Output = Qualified<T>;
        fn neg(self) -> Qualified<T> {
            match self {
                Allow(x) => Forbid(x),
                Forbid(x) => Allow(x),
            }
        }
    }

    #[test]
    fn directive_test() {
        let _everybody = parse_eval::<Spec<UserSpecifier>>("ALL");
        let _nobody = parse_eval::<Spec<UserSpecifier>>("!ALL");
        let y = |name| parse_eval::<Spec<UserSpecifier>>(name);
        let _not = |name| -parse_eval::<Spec<UserSpecifier>>(name);
        match parse_eval::<ast::Sudo>("User_Alias HENK = user1, user2") {
            Sudo::Decl(Directive::UserAlias(Def(name, list))) => {
                assert_eq!(name, "HENK");
                assert_eq!(list, vec![y("user1"), y("user2")]);
            }
            _ => panic!("incorrectly parsed"),
        }
    }

    #[test]
    // the overloading of '#' causes a lot of issues
    fn hashsign_test() {
        let Sudo::Spec(_) = parse_line("#42 ALL=ALL") else { panic!() };
        let Sudo::Spec(_) = parse_line("ALL ALL=(#42) ALL") else { panic!() };
        let Sudo::Spec(_) = parse_line("ALL ALL=(%#42) ALL") else { panic!() };
        let Sudo::Spec(_) = parse_line("ALL ALL=(:#42) ALL") else { panic!() };
        let Sudo::Decl(_) = parse_line("User_Alias FOO=#42, %#0, #3") else { panic!() };
        let Sudo::LineComment = parse_line("") else { panic!() };
        let Sudo::LineComment = parse_line("#this is a comment") else { panic!() };
        let Sudo::Include(_) = parse_line("#include foo") else { panic!() };
        let Sudo::IncludeDir(_) = parse_line("#includedir foo") else { panic!() };
        let Sudo::Include(x) = parse_line("#include \"foo bar\"") else { panic!() };
        assert_eq!(x, "foo bar");
        // this is fine
        let Sudo::LineComment = parse_line("#inlcudedir foo") else { panic!() };
        let Sudo::Include(_) = parse_line("@include foo") else { panic!() };
        let Sudo::IncludeDir(_) = parse_line("@includedir foo") else { panic!() };
        let Sudo::Include(x) = parse_line("@include \"foo bar\"") else { panic!() };
        assert_eq!(x, "foo bar");
    }

    #[test]
    #[should_panic]
    fn hashsign_error() {
        let Sudo::Include(_) = parse_line("#include foo bar") else { todo!() };
    }

    #[test]
    #[should_panic]
    fn include_regression() {
        let Sudo::Include(_) = parse_line("#4,#include foo") else { todo!() };
    }

    fn test_topo_sort(n: usize) {
        let alias = |s: &str| Qualified::Allow(Meta::<UserSpecifier>::Alias(s.to_string()));
        let stop = || Qualified::Allow(Meta::<UserSpecifier>::All);
        type Elem = Spec<UserSpecifier>;
        let test_case = |x1: Elem, x2: Elem, x3: Elem| {
            let table = vec![
                Def("AAP".to_string(), vec![x1]),
                Def("NOOT".to_string(), vec![x2]),
                Def("MIES".to_string(), vec![x3]),
            ];
            let mut err = vec![];
            let order = sanitize_alias_table(&table, &mut err);
            assert!(err.is_empty());
            let mut seen = HashSet::new();
            for Def(id, defns) in order.iter().map(|&i| &table[i]) {
                if defns.iter().any(|spec| {
                    let Qualified::Allow(Meta::Alias(id2)) = spec else { return false };
                    !seen.contains(id2)
                }) {
                    panic!("forward reference encountered after sorting");
                }
                seen.insert(id);
            }
        };
        match n {
            0 => test_case(alias("AAP"), alias("NOOT"), stop()),
            1 => test_case(alias("AAP"), stop(), alias("NOOT")),
            2 => test_case(alias("NOOT"), alias("AAP"), stop()),
            3 => test_case(alias("NOOT"), stop(), alias("AAP")),
            4 => test_case(stop(), alias("AAP"), alias("NOOT")),
            5 => test_case(stop(), alias("NOOT"), alias("AAP")),
            _ => panic!("error in test case"),
        }
    }

    #[test]
    fn test_topo_positive() {
        test_topo_sort(3);
        test_topo_sort(4);
    }

    #[test]
    #[should_panic]
    fn test_topo_fail0() {
        test_topo_sort(0);
    }
    #[test]
    #[should_panic]
    fn test_topo_fail1() {
        test_topo_sort(1);
    }
    #[test]
    #[should_panic]
    fn test_topo_fail2() {
        test_topo_sort(2);
    }
    #[test]
    #[should_panic]
    fn test_topo_fail5() {
        test_topo_sort(5);
    }

    fn fuzz_topo_sort(siz: usize) {
        for mut n in 0..(1..siz).reduce(|x, y| x * y).unwrap() {
            let name = |s: u8| std::str::from_utf8(&[65 + s]).unwrap().to_string();
            let alias = |s: String| Qualified::Allow(Meta::<UserSpecifier>::Alias(s));
            let stop = || Qualified::Allow(Meta::<UserSpecifier>::All);

            let mut data = (0..siz - 1)
                .map(|i| alias(name(i as u8)))
                .collect::<Vec<_>>();
            data.push(stop());

            for i in (1..=siz).rev() {
                let pos = n % i;
                n = n / i;
                data.swap(i - 1, pos);
            }

            let table = data
                .into_iter()
                .enumerate()
                .map(|(i, x)| Def(name(i as u8), vec![x]))
                .collect();

            let mut err = vec![];
            let order = sanitize_alias_table(&table, &mut err);
            if !err.is_empty() {
                return;
            }

            let mut seen = HashSet::new();
            for Def(id, defns) in order.iter().map(|&i| &table[i]) {
                if defns.iter().any(|spec| {
                    let Qualified::Allow(Meta::Alias(id2)) = spec else { return false };
                    !seen.contains(id2)
                }) {
                    panic!("forward reference encountered after sorting");
                }
                seen.insert(id);
            }
            assert!(seen.len() == siz);
        }
    }

    #[test]
    fn fuzz_topo_sort7() {
        fuzz_topo_sort(7)
    }
}
