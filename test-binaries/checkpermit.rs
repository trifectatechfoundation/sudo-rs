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
    request: &sudoers::Request<&str,sudoers::GroupID>,
    on_host: &str,
    chosen_poison: &str,
) {
    println!(
        "Is '{}' allowed on '{}' to run: '{}' (as {}:{:?})?",
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
    use sudoers::GroupID;
    let args: Vec<String> = env::args().collect();
    if let Ok(file) = File::open("./sudoers") {
        let cfg = io::BufReader::new(file).lines().map(|x| x.unwrap());
        println!(
            "{:?}",
            chatty_check_permission(
                cfg,
                &args[1],
                &sudoers::Request {
                    user: args.get(4).unwrap_or(&"root".to_string()),
                    group: args.get(5).map(|x|GroupID(2347,Some(x.clone()))).unwrap_or(GroupID(0,Some("root".to_owned())))
                },
                &args[2],
                &args[3],
            )
        );
    } else {
        panic!("no sudoers file!");
    }
}
