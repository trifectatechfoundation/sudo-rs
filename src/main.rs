mod ast;
mod basic_parser;
mod tokens;
use ast::*;
mod check;
use check::*;

// TODO: this should give parse error messages etc.
fn sudoers_parse(lines: impl Iterator<Item = String>) -> impl Iterator<Item = ast::PermissionSpec> {
    lines.map(|text| basic_parser::expect_complete::<PermissionSpec>(&mut text.chars().peekable()))
}

fn chatty_check_permission(
    sudoers: impl Iterator<Item = String>,
    am_user: &str,
    request: &UserInfo,
    on_host: &str,
    chosen_poison: &str,
) {
    println!(
        "Is '{}' allowed on '{}' to run: '{}' (as {}:{})?",
        am_user, on_host, chosen_poison, request.user, request.group
    );
    use ast::*;
    use tokens::*;
    let result = check_permission(
        sudoers_parse(sudoers),
        // hardcoded for now
        &AliasTable {
            user: vec![Def(
                "GROUP".to_string(),
                vec![
                    Qualified::Allow(Meta::Only(UserSpecifier::User(Username("marc".to_string())))),
                    Qualified::Allow(Meta::Only(UserSpecifier::User(Username(
                        "christian".to_string(),
                    )))),
                ],
            )],
        },
        am_user,
        request,
        on_host,
        chosen_poison,
    );
    println!("OUTCOME: {result:?}");
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
                &args[1],
                &UserInfo {
                    user: args.get(4).unwrap_or(&"root".to_string()),
                    group: args.get(5).unwrap_or(&"root".to_string())
                },
                &args[2],
                &args[3],
            )
        );
    } else {
        panic!("no sudoers file!");
    }
}
