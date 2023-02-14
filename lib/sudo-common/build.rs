use std::env;
use std::fs;
use std::path::Path;

// Return the first existing path given a list of paths as string slices
fn get_first_path(paths: &[&'static str]) -> Option<&'static str> {
    paths.iter().find(|p| Path::new(p).exists()).copied()
}

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("paths.rs");

    let path_zoneinfo: Option<&str> = get_first_path(&[
        "/usr/share/zoneinfo",
        "/usr/share/lib/zoneinfo",
        "/usr/lib/zoneinfo",
        "/usr/lib/zoneinfo",
    ]);

    let path_maildir: &str =
        get_first_path(&["/var/mail", "/var/spool/mail", "/usr/spool/mail"]).unwrap_or("/var/mail");

    let code = format!(
        "
    const PATH_MAILDIR: &str = {path_maildir:?};
    const PATH_ZONEINFO: Option<&str> = {path_zoneinfo:?};
    "
    );

    fs::write(dest_path, code).unwrap();

    println!("cargo:rerun-if-changed=build.rs");
}
