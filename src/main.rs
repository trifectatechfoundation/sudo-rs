mod ast;
mod basic_parser;
mod check;
mod tokens;

// TODO: this should give parse error messages etc.
fn sudoers_parse(lines: impl Iterator<Item = String>) -> impl Iterator<Item = ast::Sudo> {
    lines.map(
        |text| match basic_parser::expect_complete(&mut text.chars().peekable()) {
            Ok(x) => x,
            Err(error) => panic!("PARSE ERROR: {error:?}"),
        },
    )
}

fn chatty_check_permission(
    sudoers: impl Iterator<Item = String>,
    am_user: &str,
    request: &check::UserInfo,
    on_host: &str,
    chosen_poison: &str,
) {
    println!(
        "Is '{}' allowed on '{}' to run: '{}' (as {}:{})?",
        am_user, on_host, chosen_poison, request.user, request.group
    );
    let (input, aliases) = check::analyze(sudoers_parse(sudoers));
    let result =
        check::check_permission(&input, &aliases, am_user, request, on_host, chosen_poison);
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
                &check::UserInfo {
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
