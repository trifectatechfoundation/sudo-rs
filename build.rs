fn main() {
    let path_zoneinfo = [
        "/usr/share/zoneinfo",
        "/usr/share/lib/zoneinfo",
        "/usr/lib/zoneinfo",
        "/usr/lib/zoneinfo",
    ]
    .into_iter()
    .find(|p| std::path::Path::new(p).exists())
    .expect("no zoneinfo database");

    println!("cargo:rustc-env=PATH_ZONEINFO={path_zoneinfo}");
    println!("cargo:rerun-if-changed=build.rs");
}
