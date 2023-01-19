//! Code that checks (and in the future: lists) permissions in the sudoers file

use crate::ast;
use crate::ast::*;
use crate::basic_parser;
use crate::tokens::*;

/// TODO: this interface should be replaced by something that interacts with the operating system
/// Right now, we emulate that a user is always only in its own group.

fn in_group(user: &str, group: &str) -> bool {
    user == group
}

/// Find an item matching a certain predicate in an collection (optionally attributed) list of
/// identifiers; identifiers can be directly identifying, wildcards, and can either be positive or
/// negative (i.e. preceeded by an even number of exclamation marks in the sudoers file)

fn find_item<'a, Predicate, T, Permit: Tagged<T> + 'a>(
    items: impl IntoIterator<Item = &'a Permit>,
    matches: Predicate,
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
        match who {
            All::All => result = judgement.then(|| item.to_info()),
            All::Only(ident) if matches(ident) => result = judgement.then(|| item.to_info()),
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

pub struct UserInfo<'a> {
    pub user: &'a str,
    pub group: &'a str,
}

/// Check if the user [am_user] is allowed to run [cmdline] on machine [on_host] as the requested
/// user/group. Not that in the sudoers file, later permissions override earlier restrictions.
/// The [cmdline] argument should already be ready to essentially feed to an exec() call; or be
/// a special command like 'sudoedit'.

// This code is structure to allow easily reading the 'happy path'; i.e. as soon as something
// doesn't match, we escape using the '?' mechanism.
pub fn check_permission(
    sudoers: impl Iterator<Item = ast::PermissionSpec>,
    am_user: &str,
    request: &UserInfo,
    on_host: &str,
    cmdline: &str,
) -> Option<Vec<Tag>> {
    let allowed_commands = sudoers
        .filter_map(|sudo| {
            find_item(&sudo.users, match_user(am_user))?;

            let matching_rules = sudo
                .permissions
                .iter()
                .filter_map(|(hosts, runas, cmds)| {
                    find_item(hosts, match_token(on_host))?;

                    if let Some(RunAs { users, groups }) = runas {
                        if !users.is_empty() || request.user != am_user {
                            *find_item(users, match_user(request.user))?
                        }
                        if !in_group(request.user, request.group) {
                            *find_item(groups, match_token(request.group))?
                        }
                    } else if request.user != "root" || !in_group("root", request.group) {
                        None?;
                    }

                    Some(cmds)
                })
                .flatten();

            Some(matching_rules.cloned().collect::<Vec<_>>())
        })
        .collect::<Vec<_>>();

    find_item(allowed_commands.iter().flatten(), match_command(cmdline)).cloned()
}

#[cfg(test)]
mod test {
    use super::*;
    use std::iter;

    fn sudoers_parse(
        lines: impl Iterator<Item = String>,
    ) -> impl Iterator<Item = ast::PermissionSpec> {
        lines.map(|text| basic_parser::expect_complete(&mut text.chars().peekable()))
    }

    macro_rules! sudoer {
        ($h:expr $(,$e:expr)*) => {
            sudoers_parse(
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
        let string = "ALL ALL = (;) ALL";
        basic_parser::expect_nonterminal::<ast::Sudo>(&mut string.chars().peekable());
    }

    #[test]
    fn ambiguous_spec1() {
        let string = "marc, User_Alias ALL = ALL";
        let Sudo::Spec(_) = basic_parser::expect_nonterminal::<ast::Sudo>(&mut string.chars().peekable()) else { todo!() };
    }

    #[test]
    fn ambiguous_spec2() {
        let string = "User_Alias ALIAS = ALL";
        let Sudo::Decl(_) = basic_parser::expect_nonterminal::<ast::Sudo>(&mut string.chars().peekable()) else { todo!() };
    }

    #[test]
    #[should_panic]
    fn ambiguous_spec3() {
        let string = "User_Alias, marc ALL = ALL";
        basic_parser::expect_nonterminal::<ast::Sudo>(&mut string.chars().peekable());
    }

    #[test]
    #[rustfmt::skip]
    fn permission_test() {
        let root = UserInfo {
            user: "root",
            group: "root",
        };
        assert_eq!(check_permission(sudoer!("user ALL=(ALL:ALL) ALL"), "nobody", &root, "server", "/bin/hello"), None);
        assert_eq!(check_permission(sudoer!("user ALL=(ALL:ALL) ALL"), "user",   &root, "server", "/bin/hello"), Some(vec![]));
        assert_eq!(check_permission(sudoer!("user ALL=(ALL:ALL) /bin/foo"), "user",   &root, "server", "/bin/foo"), Some(vec![]));
        assert_eq!(check_permission(sudoer!("user ALL=(ALL:ALL) /bin/foo"), "user",   &root, "server", "/bin/hello"), None);
        assert_eq!(check_permission(sudoer!("user ALL=(ALL:ALL) /bin/foo, NOPASSWD: /bin/bar"), "user",   &root, "server", "/bin/foo"), Some(vec![]));
        assert_eq!(check_permission(sudoer!("user ALL=(ALL:ALL) /bin/foo, NOPASSWD: /bin/bar"), "user",   &root, "server", "/bin/bar"), Some(vec![Tag::NOPASSWD]));
        assert_eq!(check_permission(sudoer!("user server=(ALL:ALL) ALL"), "user", &root, "server", "/bin/hello"), Some(vec![]));
        assert_eq!(check_permission(sudoer!("user laptop=(ALL:ALL) ALL"), "user", &root, "server", "/bin/hello"), None);
        assert_eq!(check_permission(sudoer!["user ALL=!/bin/hello",
                                            "user ALL=/bin/hello"], "user",   &root, "server", "/bin/hello"), Some(vec![]));
        assert_eq!(check_permission(sudoer!["user ALL=/bin/hello",
                                            "user ALL=!/bin/hello"], "user",   &root, "server", "/bin/hello"), None);
        assert_eq!(check_permission(sudoer!["user ALL=/bin/hello",
                                            "user ALL=!/bin/whoami"], "user",   &root, "server", "/bin/hello"), Some(vec![]));
    }

    #[test]
    #[should_panic]
    fn invalid_directive() {
        let string = "User_Alias, user Alias = user1, user2";
        basic_parser::expect_nonterminal::<ast::Sudo>(&mut string.chars().peekable());
    }

    #[test]
    fn directive_test() {
        let _everybody = Qualified::Allow(All::<UserSpecifier>::All);
        let _nobody = Qualified::Forbid(All::<UserSpecifier>::All);
        let y = |name: &str| Qualified::Allow(All::Only(UserSpecifier::User(Username(name.to_owned()))));
        let _not = |name: &str| Qualified::Forbid(All::Only(name.to_owned()));
        match basic_parser::expect_nonterminal::<ast::Sudo>(
            &mut "User_Alias HENK = user1, user2".chars().peekable(),
        ) {
            Sudo::Decl(Directive::UserAlias(name, list)) => {
                assert_eq!(name, "HENK");
                assert_eq!(list, vec![y("user1"), y("user2")]);
            }
            _ => panic!("incorrectly parsed"),
        }
    }
}
