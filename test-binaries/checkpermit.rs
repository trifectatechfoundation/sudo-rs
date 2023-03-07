use sudo_common::sysuser::{UnixGroup, UnixUser};

fn chatty_check_permission(
    sudoers: sudoers::Sudoers,
    am_user: &str,
    request: sudoers::Request<&str, GroupID>,
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

/// This is the "canonical" info that we need
#[derive(Debug)]
pub struct GroupID(pub libc::gid_t, pub Option<String>);

impl UnixGroup for GroupID {
    fn as_gid(&self) -> libc::gid_t {
        self.0
    }

    fn try_as_name(&self) -> Option<&str> {
        self.1.as_deref()
    }
}

impl UnixUser for GroupID {
    fn has_uid(&self, uid: libc::gid_t) -> bool {
        self.0 == uid
    }
    fn has_name(&self, name: &str) -> bool {
        self.1.as_ref().map_or(false, |s| s == name)
    }
}

#[derive(Debug)]
pub struct UserRecord(pub libc::gid_t, pub Option<String>, pub Vec<GroupID>);

impl PartialEq<UserRecord> for UserRecord {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl Eq for UserRecord {}

impl UnixUser for UserRecord {
    fn is_root(&self) -> bool {
        self.has_name("root") && self.has_uid(0)
    }

    fn in_group_by_name(&self, name: &str) -> bool {
        self.2.iter().any(|g| g.has_name(name))
    }

    fn in_group_by_gid(&self, id: libc::gid_t) -> bool {
        self.2.iter().any(|g| g.has_uid(id))
    }
}

fn fancy_error(x: usize, y: usize, path: &str) {
    use std::io::*;
    let inp = BufReader::new(std::fs::File::open(path).unwrap());
    let line = inp.lines().nth(x - 1).unwrap().unwrap();
    eprintln!("{line}");
    for (i, c) in line.chars().enumerate() {
        if i == y - 1 {
            break;
        }
        if c.is_whitespace() {
            eprint!("{c}");
        } else {
            eprint!(" ");
        }
    }
    eprintln!("^");
}

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Ok((cfg, warn)) = sudoers::compile("./sudoers") {
        for sudoers::Error(pos, msg) in warn {
            if let Some((x, y)) = pos {
                fancy_error(x, y, "./sudoers");
            }
            eprintln!("{msg}");
        }
        println!("SETTINGS: {:?}", cfg.settings);
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
