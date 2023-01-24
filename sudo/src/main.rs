use sudo_system::{hostname, Group, Process, User};

fn main() {
    let hostname = hostname();
    let user = User::effective();
    let real_user = User::real();
    let group = Group::effective();
    let real_group = Group::real();
    let process_info = Process::new();
    println!("{:?}", hostname);
    println!("{:?}", user);
    println!("{:?}", real_user);
    println!("{:?}", group);
    println!("{:?}", real_group);
    println!("{:?}", process_info);
}
