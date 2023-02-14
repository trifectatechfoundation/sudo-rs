fn chatty_check_permission(
    sudoers: sudoers::Sudoers,
    am_user: &str,
    request: sudoers::Request<&str, sudoers::GroupID>,
    on_host: &str,
    chosen_poison: &str,
) {
    println!(
        "Is '{}' allowed on '{}' to run: '{}' (as {}:{:?})?",
        am_user, on_host, chosen_poison, request.user, request.group
    );
    let result = sudoers::check_permission(&sudoers, &am_user, request, on_host, chosen_poison);
    println!("OUTCOME: {result:?}");
}

use std::env;

fn main() {
    use sudoers::GroupID;
    let args: Vec<String> = env::args().collect();
    if let Ok((cfg, warn)) = sudoers::compile("./sudoers") {
        for foobar in warn {
            println!("ERROR: {foobar:?}")
        }
        println!(
            "{:?}",
            chatty_check_permission(
                cfg,
                &args[1],
                sudoers::Request::<&str, GroupID> {
                    user: &args.get(4).unwrap_or(&"root".to_owned()).as_str(),
                    group: &args
                        .get(5)
                        .map(|x| GroupID(2347, Some(x.clone())))
                        .unwrap_or_else(|| (GroupID(0, Some("root".to_owned()))))
                },
                &args[2],
                &args[3],
            )
        );
    } else {
        panic!("no sudoers file!");
    }
}
