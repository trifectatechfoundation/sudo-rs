mod ast;
mod basic_parser;
mod tokens;
use ast::*;
use tokens::*;

fn match_item<Predicate, T, Permit: Tagged<Spec<T>>>(
    items: &Vec<Permit>,
    matches: Predicate,
) -> Option<&Permit::Flags>
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

fn exact<T: Eq + ?Sized>(s1: &T) -> (impl Fn(&T) -> bool + '_) {
    move |s2| s1 == s2
}

struct UserInfo<'a> {
    user: &'a str,
    group: &'a str,
}

// this interface should use a type that also supports other ways of specifying users and groups
fn in_group(user: &str, group: &str) -> bool {
    user == group
}

fn check_user(username: &str) -> (impl Fn(&UserSpecifier) -> bool + '_) {
    move |spec| match spec {
        UserSpecifier::User(name) => name.0 == username,
        UserSpecifier::Group(groupname) => in_group(username, groupname.0.as_str()),
    }
}

fn check_permission(
    sudoers: impl Iterator<Item = String>,
    am: UserInfo,
    request: UserInfo,
    on_host: &str,
    cmdline: &str,
) -> Option<<CommandSpec as Tagged<Spec<Command>>>::Flags> {
    sudoers
        .filter_map(|text| {
            let sudo = basic_parser::expect_complete::<Sudo>(&mut text.chars().peekable());

            match_item(&sudo.users, check_user(am.user))?;

            let matching_rules = sudo.permissions.iter().filter_map(|(hosts, runas, cmds)| {
                match_item(hosts, exact(&tokens::Hostname(on_host.to_string())))?;
                if let Some(RunAs { users, groups }) = runas {
                    if !users.is_empty() {
                        match_item(users, exact(&tokens::Username(request.user.to_string())))
                    } else {
                        (request.user == am.user).then_some(NO_TAG)
                    }?;
                    if !groups.is_empty() {
                        match_item(groups, exact(&tokens::Username(request.group.to_string())))
                    } else {
                        in_group(request.user, request.group).then_some(NO_TAG)
                    }?;
                } else if request.user != "root" || !in_group("root", request.group) {
                    None?;
                }
                match_item(cmds, exact(&tokens::Command(cmdline.to_string())))
            });

            matching_rules.last().cloned()
        })
        .last()
}

fn chatty_check_permission(
    sudoers: impl Iterator<Item = String>,
    am: UserInfo,
    request: UserInfo,
    on_host: &str,
    chosen_poison: &str,
) {
    println!(
        "Is '{}:{}' allowed on '{}' to run: '{}' (as {}:{})?",
        am.user, am.group, on_host, chosen_poison, request.user, request.group
    );
    let result = check_permission(sudoers, am, request, on_host, chosen_poison);
    println!("OUTCOME: {:?}", result);
}

use std::env;
use std::fs::File;
use std::io::{self, BufRead};

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Ok(file) = File::open("./sudoers") {
        let cfg = io::BufReader::new(file).lines().map(|x| x.unwrap());
        println!(
            "{:?}",
            chatty_check_permission(
                cfg,
                UserInfo {
                    user: &args[1],
                    group: &args[2]
                },
                UserInfo {
                    user: args.get(5).unwrap_or(&"root".to_string()),
                    group: args.get(6).unwrap_or(&"root".to_string())
                },
                &args[3],
                &args[4],
            )
        );
    } else {
        panic!("no sudoers file!");
    }
}
