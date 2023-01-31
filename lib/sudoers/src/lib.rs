//TODO: sanitize_...() panics when it finds errors; it should emit warnings instead and come to some solution that is conservative

//! Code that checks (and in the future: lists) permissions in the sudoers file

mod ast;
mod basic_parser;
mod tokens;

use std::collections::HashSet;

use ast::*;
use tokens::*;

/// Export some necessary symbols from modules
pub use ast::Sudo;
pub use ast::Tag;
pub use basic_parser::parse_string;

/// TODO: this interface should be replaced by something that interacts with the operating system
/// Right now, we emulate that a user is always only in its own group.

fn in_group(user: &str, group: &str) -> bool {
    user == group
}

pub struct UserInfo<'a> {
    pub user: &'a str,
    pub group: &'a str,
}

// TODO: combine this with Vec<PermissionSpec> into a single data structure?
#[derive(Default)]
pub struct AliasTable {
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
pub fn check_permission<'a>(
    sudoers: impl IntoIterator<Item = &'a PermissionSpec>,
    alias_table: &AliasTable,
    am_user: &str,
    request: &UserInfo,
    on_host: &str,
    cmdline: &str,
) -> Option<Vec<Tag>> {
    let user_aliases = get_aliases(&alias_table.user, &match_user(am_user));
    let host_aliases = get_aliases(&alias_table.host, &match_token(on_host));
    let cmnd_aliases = get_aliases(&alias_table.cmnd, &match_command(cmdline));
    let runas_user_aliases = get_aliases(&alias_table.runas, &match_user(request.user));
    let runas_group_aliases = get_aliases(&alias_table.runas, &match_group_alias(request.group));

    let allowed_commands = sudoers
        .into_iter()
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
                            *find_item(groups, &match_token(request.group), &runas_group_aliases)?
                        }
                    } else if request.user != "root" || !in_group("root", request.group) {
                        None?;
                    }

                    Some(cmds)
                })
                .flatten();

            Some(matching_rules.collect::<Vec<_>>())
        })
        .flatten();

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

fn match_user(username: &str) -> (impl Fn(&UserSpecifier) -> bool + '_) {
    move |spec| match spec {
        UserSpecifier::User(name) => name.0 == username,
        UserSpecifier::Group(groupname) => in_group(username, groupname.0.as_str()),
    }
}

fn match_group_alias(groupname: &str) -> (impl Fn(&UserSpecifier) -> bool + '_) {
    move |spec| match spec {
        UserSpecifier::User(name) => name.0 == groupname,
        /* the parser rejects this, but can happen due to Runas_Alias,
         * see https://github.com/memorysafety/sudo-rs/issues/13 */
        UserSpecifier::Group(_) => {
            eprintln!("warning: ignoring %group syntax for use sudo -g");
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

/// Process a sudoers-parsing file into a workable AST

pub fn analyze(sudoers: impl IntoIterator<Item = Sudo>) -> (Vec<PermissionSpec>, AliasTable) {
    use Directive::*;
    let mut permits = Vec::new();
    let mut alias: AliasTable = Default::default();
    for item in sudoers {
        match item {
            Sudo::Spec(permission) => permits.push(permission),
            Sudo::Decl(UserAlias(def)) => alias.user.1.push(def),
            Sudo::Decl(HostAlias(def)) => alias.host.1.push(def),
            Sudo::Decl(CmndAlias(def)) => alias.cmnd.1.push(def),
            Sudo::Decl(RunasAlias(def)) => alias.runas.1.push(def),
        }
    }

    alias.user.0 = sanitize_alias_table(&alias.user.1);
    alias.host.0 = sanitize_alias_table(&alias.host.1);
    alias.cmnd.0 = sanitize_alias_table(&alias.cmnd.1);
    alias.runas.0 = sanitize_alias_table(&alias.runas.1);
    (permits, alias)
}

/// Alias definition inin a Sudoers file can come in any order; and aliases can refer to other aliases, etc.
/// It is much easier if they are presented in a "definitional order" (i.e. aliases that use other aliases occur later)
/// At the same time, this is a good place to detect problems in the aliases, such as unknown aliases and cycles.

fn sanitize_alias_table<T>(table: &Vec<Def<T>>) -> Vec<usize> {
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
    }

    impl<T> Visitor<'_, T> {
        fn visit(&mut self, pos: usize) {
            if self.seen.insert(pos) {
                let Def(_, members) = &self.table[pos];
                for elem in members {
                    let Meta::Alias(name) = remqualify(elem) else { break };
                    let Some(dependency) = self.table.iter().position(|Def(id,_)| id==name) else {
			panic!("undefined alias: `{name}'");
		    };
                    self.visit(dependency);
                }
                self.order.push(pos);
            } else if !self.order.contains(&pos) {
                let Def(id, _) = &self.table[pos];
                panic!("recursive alias: `{id}'");
            }
        }
    }

    let mut visitor = Visitor {
        seen: HashSet::new(),
        table,
        order: Vec::with_capacity(table.len()),
    };

    let mut dupe = HashSet::new();
    for (i, Def(name, _)) in table.iter().enumerate() {
        if !dupe.insert(name) {
            panic!("multiple occurences of `{name}'");
        } else {
            visitor.visit(i);
        }
    }

    visitor.order
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast;
    use basic_parser::parse_eval;
    use std::iter;

    macro_rules! sudoer {
        ($h:expr $(,$e:expr)*) => {
            (
                iter::once($h)
                $(
                    .chain(iter::once($e))
                )*
            ).map(parse_eval)
        }
    }

    #[test]
    #[should_panic]
    fn invalid_spec() {
        parse_eval::<ast::Sudo>("ALL ALL = (;) ALL");
    }

    #[test]
    fn ambiguous_spec1() {
        let Sudo::Spec(_) = parse_eval::<ast::Sudo>("marc, User_Alias ALL = ALL") else { todo!() };
    }

    #[test]
    fn ambiguous_spec2() {
        let Sudo::Decl(_) = parse_eval::<ast::Sudo>("User_Alias ALIAS = ALL") else { todo!() };
    }

    #[test]
    #[should_panic]
    fn ambiguous_spec3() {
        parse_eval::<ast::Sudo>("User_Alias, marc ALL = ALL");
    }

    #[test]
    fn permission_test() {
        let root = UserInfo {
            user: "root",
            group: "root",
        };

        macro_rules! FAIL {
            ([$($sudo:expr),*], $user:expr => $req:expr, $server:expr; $command:expr) => {
                let (input,alias) = analyze(sudoer![$($sudo),*]);
                assert_eq!(check_permission(&input, &alias, $user, $req, $server, $command), None);
            }
        }

        macro_rules! pass {
            ([$($sudo:expr),*], $user:expr => $req:expr, $server:expr; $command:expr $(=> [$($list:expr),*])?) => {
                let (input,alias) = analyze(sudoer![$($sudo),*]);
                let result = check_permission(&input, &alias, $user, $req, $server, $command);
                $(assert_eq!(result, Some(vec![$($list),*]));)?
                assert!(!result.is_none());
            }
        }
        use crate::ast::Tag::*;

        FAIL!(["user ALL=(ALL:ALL) ALL"], "nobody"    => &root, "server"; "/bin/hello");
        pass!(["user ALL=(ALL:ALL) ALL"], "user"      => &root, "server"; "/bin/hello");
        pass!(["user ALL=(ALL:ALL) /bin/foo"], "user" => &root, "server"; "/bin/foo");
        FAIL!(["user ALL=(ALL:ALL) /bin/foo"], "user" => &root, "server"; "/bin/hello");
        pass!(["user ALL=(ALL:ALL) /bin/foo, NOPASSWD: /bin/bar"], "user" => &root, "server"; "/bin/foo");
        pass!(["user ALL=(ALL:ALL) /bin/foo, NOPASSWD: /bin/bar"], "user" => &root, "server"; "/bin/bar" => [NoPasswd]);

        pass!(["user server=(ALL:ALL) ALL"], "user" => &root, "server"; "/bin/hello");
        FAIL!(["user laptop=(ALL:ALL) ALL"], "user" => &root, "server"; "/bin/hello");

        pass!(["user ALL=!/bin/hello", "user ALL=/bin/hello"], "user" => &root, "server"; "/bin/hello");
        FAIL!(["user ALL=/bin/hello", "user ALL=!/bin/hello"], "user" => &root, "server"; "/bin/hello");

        for alias in [
            "User_Alias GROUP=user1, user2",
            "User_Alias GROUP=ALL,!user3",
        ] {
            pass!([alias,"GROUP ALL=/bin/hello"], "user1" => &root, "server"; "/bin/hello");
            pass!([alias,"GROUP ALL=/bin/hello"], "user2" => &root, "server"; "/bin/hello");
            FAIL!([alias,"GROUP ALL=/bin/hello"], "user3" => &root, "server"; "/bin/hello");
        }
        pass!(["user ALL=/bin/hello arg"], "user" => &root, "server"; "/bin/hello arg");
        pass!(["user ALL=/bin/hello  arg"], "user" => &root, "server"; "/bin/hello arg");
        pass!(["user ALL=/bin/hello arg"], "user" => &root, "server"; "/bin/hello  arg");
        FAIL!(["user ALL=/bin/hello arg"], "user" => &root, "server"; "/bin/hello boo");
        pass!(["user ALL=/bin/hello a*g"], "user" => &root, "server"; "/bin/hello  aaaarg");
        FAIL!(["user ALL=/bin/hello a*g"], "user" => &root, "server"; "/bin/hello boo");
        pass!(["user ALL=/bin/hello"], "user" => &root, "server"; "/bin/hello boo");
        FAIL!(["user ALL=/bin/hello \"\""], "user" => &root, "server"; "/bin/hello boo");
        pass!(["user ALL=/bin/hello \"\""], "user" => &root, "server"; "/bin/hello");
        pass!(["user ALL=/bin/hel*"], "user" => &root, "server"; "/bin/hello");
        pass!(["user ALL=/bin/hel*"], "user" => &root, "server"; "/bin/help");
        pass!(["user ALL=/bin/hel*"], "user" => &root, "server"; "/bin/help me");
        pass!(["user ALL=/bin/hel* *"], "user" => &root, "server"; "/bin/help");
        FAIL!(["user ALL=/bin/hel* me"], "user" => &root, "server"; "/bin/help");
        pass!(["user ALL=/bin/hel* me"], "user" => &root, "server"; "/bin/help me");
        FAIL!(["user ALL=/bin/hel* me"], "user" => &root, "server"; "/bin/help me please");

        pass!(["User_Alias FULLTIME=ALL,!marc","FULLTIME ALL=ALL"], "user" => &root, "server"; "/bin/bash");
        FAIL!(["User_Alias FULLTIME=ALL,!marc","FULLTIME ALL=ALL"], "marc" => &root, "server"; "/bin/bash");
        FAIL!(["User_Alias FULLTIME=ALL,!marc","ALL,!FULLTIME ALL=ALL"], "user" => &root, "server"; "/bin/bash");
        pass!(["User_Alias FULLTIME=ALL,!marc","ALL,!FULLTIME ALL=ALL"], "marc" => &root, "server"; "/bin/bash");
        pass!(["Host_Alias MACHINE=laptop,server","user MACHINE=ALL"], "user" => &root, "server"; "/bin/bash");
        pass!(["Host_Alias MACHINE=laptop,server","user MACHINE=ALL"], "user" => &root, "laptop"; "/bin/bash");
        FAIL!(["Host_Alias MACHINE=laptop,server","user MACHINE=ALL"], "user" => &root, "desktop"; "/bin/bash");
        pass!(["Cmnd_Alias WHAT=/bin/dd, /bin/rm","user ALL=WHAT"], "user" => &root, "server"; "/bin/rm");
        pass!(["Cmd_Alias WHAT=/bin/dd,/bin/rm","user ALL=WHAT"], "user" => &root, "laptop"; "/bin/dd");
        FAIL!(["Cmnd_Alias WHAT=/bin/dd,/bin/rm","user ALL=WHAT"], "user" => &root, "desktop"; "/bin/bash");

        pass!(["User_Alias A=B","User_Alias B=user","A ALL=ALL"], "user" => &root, "vm"; "/bin/ls");
        pass!(["Host_Alias A=B","Host_Alias B=vm","ALL A=ALL"], "user" => &root, "vm"; "/bin/ls");
        pass!(["Cmnd_Alias A=B","Cmnd_Alias B=/bin/ls","ALL ALL=A"], "user" => &root, "vm"; "/bin/ls");

        FAIL!(["Runas_Alias TIME=%wheel,sudo","user ALL=() ALL"], "user" => &UserInfo{ user: "sudo", group: "sudo" }, "vm"; "/bin/ls");
        pass!(["Runas_Alias TIME=%wheel,sudo","user ALL=(TIME) ALL"], "user" => &UserInfo{ user: "sudo", group: "sudo" }, "vm"; "/bin/ls");
        FAIL!(["Runas_Alias TIME=%wheel,sudo","user ALL=(:TIME) ALL"], "user" => &UserInfo{ user: "sudo", group: "sudo" }, "vm"; "/bin/ls");
        pass!(["Runas_Alias TIME=%wheel,sudo","user ALL=(:TIME) ALL"], "user" => &UserInfo{ user: "user", group: "sudo" }, "vm"; "/bin/ls");
        pass!(["Runas_Alias TIME=%wheel,sudo","user ALL=(TIME) ALL"], "user" => &UserInfo{ user: "wheel", group: "wheel" }, "vm"; "/bin/ls");
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
            let order = sanitize_alias_table(&table);
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

            let Ok(order) = std::panic::catch_unwind(||
	        sanitize_alias_table(&table)
            ) else { return; };

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
