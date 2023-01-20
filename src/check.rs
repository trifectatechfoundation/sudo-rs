//! Code that checks (and in the future: lists) permissions in the sudoers file

use std::collections::HashSet;

use crate::ast::*;
use crate::basic_parser;
use crate::tokens::*;

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
    user: Vec<Def<UserSpecifier>>,
}

/// Process a sudoers-parsing file into a workable AST

pub fn analyze(sudoers: impl Iterator<Item = Sudo>) -> (Vec<PermissionSpec>, AliasTable) {
    use Directive::*;
    let mut permits = Vec::new();
    let mut alias: AliasTable = Default::default();
    for item in sudoers {
        match item {
            Sudo::Spec(permission) => permits.push(permission),
            Sudo::Decl(UserAlias(def)) => alias.user.push(def),
        }
    }

    sanitize_alias_table(&mut alias.user);
    (permits, alias)
}

/// Check if the user [am_user] is allowed to run [cmdline] on machine [on_host] as the requested
/// user/group. Not that in the sudoers file, later permissions override earlier restrictions.
/// The [cmdline] argument should already be ready to essentially feed to an exec() call; or be
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
    let runas_aliases = HashSet::new();
    let host_aliases = HashSet::new();
    let cmnd_aliases = HashSet::new();

    let allowed_commands = sudoers
        .into_iter()
        .filter_map(|sudo| {
            find_item(&sudo.users, &match_user(am_user), &user_aliases)?;

            let matching_rules = sudo
                .permissions
                .iter()
                .filter_map(|(hosts, runas, cmds)| {
                    find_item(hosts, &match_token(on_host), &host_aliases)?;

                    //TODO: investigate the role of runas_aliases; can these contain both groups AND users? is user_alias involved here?
                    if let Some(RunAs { users, groups }) = runas {
                        if !users.is_empty() || request.user != am_user {
                            *find_item(users, &match_user(request.user), &runas_aliases)?
                        }
                        if !in_group(request.user, request.group) {
                            *find_item(groups, &match_token(request.group), &runas_aliases)?
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
            Qualified::Forbid(x) => (false, x),
            Qualified::Allow(x) => (true, x),
        };
        let get_flags = || item.to_info();
        match who {
            Meta::All => result = judgement.then(get_flags),
            Meta::Only(ident) if matches(ident) => result = judgement.then(get_flags),
            Meta::Alias(id) if aliases.contains(id) => result = judgement.then(get_flags),
            _ => {}
        };
    }
    result
}

#[allow(dead_code)]
/// A predicate that matches using "==".
fn exact<T: Eq + ?Sized>(s1: &T) -> (impl Fn(&T) -> bool + '_) {
    move |s2| s1 == s2
}

fn match_user(username: &str) -> (impl Fn(&UserSpecifier) -> bool + '_) {
    move |spec| match spec {
        UserSpecifier::User(name) => name.0 == username,
        UserSpecifier::Group(groupname) => in_group(username, groupname.0.as_str()),
    }
}

fn match_token<T: basic_parser::Token + std::ops::Deref<Target = String>>(
    text: &str,
) -> (impl Fn(&T) -> bool + '_) {
    move |token| token.as_str() == text
}

/// TODO: this should use globbing,
fn match_command<T: basic_parser::Token + std::ops::Deref<Target = String>>(
    text: &str,
) -> (impl Fn(&T) -> bool + '_) {
    move |token| token.as_str() == text
}

/// Find all the aliases that a object is a member of; this requires [sanitized_alias_table] to have run first;
/// I.e. this function should not be "pub".

fn get_aliases<Predicate, T>(table: &Vec<Def<T>>, pred: &Predicate) -> HashSet<String>
where
    Predicate: Fn(&T) -> bool,
{
    let mut set = HashSet::new();
    for Def(id, list) in table {
        if find_item(list, &pred, &set).is_some() {
            set.insert(id.clone());
        }
    }

    set
}

/// Alias definition inin a Sudoers file can come in any order; and aliases can refer to other aliases, etc.
/// It is much easier if they are presented in a "definitional order" (i.e. aliases that use other aliases occur later)
/// At the same time, this is a good place to detect problems in the aliases, such as unknown aliases and

fn sanitize_alias_table<T>(table: &mut Vec<Def<T>>) {
    fn remqualify<U>(item: &Qualified<U>) -> &U {
        match item {
            Qualified::Allow(x) => x,
            Qualified::Forbid(x) => x,
        }
    }

    let derange = {
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
    };

    // now swap the original array into the correct form
    let mut xlat = (0..table.len()).collect::<Vec<_>>();
    let mut oldp = (0..table.len()).collect::<Vec<_>>();

    for i in 0..table.len() {
        let new_i = xlat[derange[i]];
        table.swap(i, new_i);
        xlat[oldp[i]] = new_i;
        oldp[new_i] = oldp[i];
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast;
    use basic_parser::parse_eval;
    use std::iter;

    fn sudoers_parse(lines: impl Iterator<Item = String>) -> Vec<ast::PermissionSpec> {
        lines
            .map(|text| basic_parser::expect_complete(&mut text.chars().peekable()))
            .collect()
    }

    macro_rules! sudoer {
        ($h:expr $(,$e:expr)*) => {
            &sudoers_parse(
                iter::once($h)
                $(
                    .chain(iter::once($e))
                )*
                .map(str::to_string)
            )
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
        let no_alias = &AliasTable { user: Vec::new() };

        macro_rules! FAIL {
	    ([$($sudo:expr),*], $alias:expr, $user:expr => $req:expr, $server:expr; $command:expr) => {
		assert_eq!(check_permission(sudoer![$($sudo),*], $alias, $user, $req, $server, $command), None);
	    }
	}

        macro_rules! pass {
	    ([$($sudo:expr),*], $alias:expr, $user:expr => $req:expr, $server:expr; $command:expr => [$($list:expr),*]) => {
		assert_eq!(check_permission(sudoer![$($sudo),*], $alias, $user, $req, $server, $command), Some(vec![$($list),*]));
	    }
	}
        use crate::ast::Tag::*;

        FAIL!(["user ALL=(ALL:ALL) ALL"], no_alias, "nobody"    => &root, "server"; "/bin/hello");
        pass!(["user ALL=(ALL:ALL) ALL"], no_alias, "user"      => &root, "server"; "/bin/hello" => []);
        pass!(["user ALL=(ALL:ALL) /bin/foo"], no_alias, "user" => &root, "server"; "/bin/foo" => []);
        FAIL!(["user ALL=(ALL:ALL) /bin/foo"], no_alias, "user" => &root, "server"; "/bin/hello");
        pass!(["user ALL=(ALL:ALL) /bin/foo, NOPASSWD: /bin/bar"], no_alias, "user" => &root, "server"; "/bin/foo" => []);
        pass!(["user ALL=(ALL:ALL) /bin/foo, NOPASSWD: /bin/bar"], no_alias, "user" => &root, "server"; "/bin/bar" => [NOPASSWD]);

        pass!(["user server=(ALL:ALL) ALL"], no_alias, "user" => &root, "server"; "/bin/hello" => []);
        FAIL!(["user laptop=(ALL:ALL) ALL"], no_alias, "user" => &root, "server"; "/bin/hello");

        pass!(["user ALL=!/bin/hello", "user ALL=/bin/hello"], no_alias, "user" => &root, "server"; "/bin/hello" => []);
        FAIL!(["user ALL=/bin/hello", "user ALL=!/bin/hello"], no_alias, "user" => &root, "server"; "/bin/hello");

        for alias in &[
            AliasTable {
                user: vec![Def("GROUP".to_string(), parse_eval("user1, user2"))],
            },
            AliasTable {
                user: vec![Def("GROUP".to_string(), parse_eval("ALL,!user3"))],
            },
        ] {
            pass!(["GROUP ALL=/bin/hello"], alias, "user1" => &root, "server"; "/bin/hello" => []);
            pass!(["GROUP ALL=/bin/hello"], alias, "user2" => &root, "server"; "/bin/hello" => []);
            FAIL!(["GROUP ALL=/bin/hello"], alias, "user3" => &root, "server"; "/bin/hello");
        }
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
            let mut table = vec![
                Def("AAP".to_string(), vec![x1]),
                Def("NOOT".to_string(), vec![x2]),
                Def("MIES".to_string(), vec![x3]),
            ];
            sanitize_alias_table(&mut table);
            let mut seen = HashSet::new();
            for Def(id, defns) in table {
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
    fn test_topo_positve() {
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
}
