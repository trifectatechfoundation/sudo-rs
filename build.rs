use std::path::Path;

// Return the first existing path given a list of paths as string slices
fn get_first_path(paths: &[&'static str]) -> Option<&'static str> {
    paths.iter().find(|p| Path::new(p).exists()).copied()
}

fn main() {
    let path_zoneinfo: &str = get_first_path(&[
        "/usr/share/zoneinfo",
        "/usr/share/lib/zoneinfo",
        "/usr/lib/zoneinfo",
        "/usr/lib/zoneinfo",
    ])
    .unwrap_or("");

    let path_maildir: &str =
        get_first_path(&["/var/mail", "/var/spool/mail", "/usr/spool/mail"]).unwrap_or("/var/mail");

    // TODO: use _PATH_STDPATH and _PATH_DEFPATH_ROOT from paths.h
    println!("cargo:rustc-env=SUDO_PATH_DEFAULT=/usr/bin:/bin:/usr/sbin:/sbin");
    println!(
        "cargo:rustc-env=SU_PATH_DEFAULT_ROOT=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
    );
    println!(
        "cargo:rustc-env=SU_PATH_DEFAULT=/usr/local/bin:/usr/bin:/bin:/usr/local/games:/usr/games"
    );

    println!("cargo:rustc-env=PATH_MAILDIR={path_maildir}");
    println!("cargo:rustc-env=PATH_ZONEINFO={path_zoneinfo}");
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rustc-link-lib=pam");
}
