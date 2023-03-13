use std::env;
use sudo_system::interface::{UnixGroup, UnixUser};
use sudoers::Sudoers;

fn chatty_check_permission(
    sudoers: sudoers::Sudoers,
    am_user: UserRecord,
    (user, group): (UserRecord, GroupID),
    on_host: &str,
    chosen_poison: &str,
) {
    println!(
        "Is '{}' allowed on '{}' to run: '{}' (as {}:{})?",
        am_user, on_host, chosen_poison, user, group
    );
    let (command, arguments) = {
        let mut items = chosen_poison.split_whitespace();
        (items.next().unwrap(), items.collect::<Vec<_>>().join(" "))
    };
    let result = sudoers.check(
        &am_user,
        on_host,
        sudoers::Request {
            user: &user,
            group: &group,
            command: command.as_ref(),
            arguments: &arguments,
        },
    );
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
    fn has_uid(&self, uid: libc::gid_t) -> bool {
        self.0 == uid
    }

    fn has_name(&self, name: &str) -> bool {
        self.1.as_ref().map_or(false, |s| s == name)
    }

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

impl std::fmt::Display for GroupID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let default = "<UNNAMED>".to_string();
        write!(f, "{}(#{})", self.1.as_ref().unwrap_or(&default), self.0)
    }
}

impl std::fmt::Display for UserRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let default = "<UNNAMED>".to_string();
        write!(f, "{}(#{})", self.1.as_ref().unwrap_or(&default), self.0)?;
        write!(f, "[")?;
        for g in &self.2 {
            write!(f, "{g}")?;
        }
        write!(f, "]")
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

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Ok((cfg, warn)) = Sudoers::new("./sudoers") {
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
                UserRecord(12314, Some(args[1].clone()), vec![]),
                (
                    UserRecord(
                        8123,
                        Some(args.get(4).unwrap_or(&"root".to_owned()).to_string()),
                        vec![]
                    ),
                    args.get(5)
                        .map(|x| GroupID(2347, Some(x.clone())))
                        .unwrap_or_else(|| (GroupID(0, Some("root".to_owned()))))
                ),
                &args[2],
                &args[3],
            )
        );
    } else {
        panic!("no sudoers file!");
    }
}
