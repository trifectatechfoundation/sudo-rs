// TODO: this should give parse error messages etc.
fn sudoers_parse(lines: impl Iterator<Item = String>) -> impl Iterator<Item = sudoers::Sudo> {
    lines.filter_map(|text| match sudoers::parse_string(&text) {
        Ok(x) => Some(x),
        Err(error) => {
            eprintln!("PARSE ERROR: {error:?}");
            None
        }
    })
}

fn chatty_check_permission(
    sudoers: impl Iterator<Item = String>,
    am_user: &str,
    request: &sudoers::UserInfo,
    on_host: &str,
    chosen_poison: &str,
) {
    println!(
        "Is '{}' allowed on '{}' to run: '{}' (as {}:{})?",
        am_user, on_host, chosen_poison, request.user, request.group
    );
    let (input, aliases) = sudoers::analyze(sudoers_parse(sudoers));
    let result =
        sudoers::check_permission(&input, &aliases, am_user, request, on_host, chosen_poison);
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
                &sudoers::UserInfo {
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
