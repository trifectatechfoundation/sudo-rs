#[link(name = "apparmor")]
extern "C" {
    pub fn aa_change_onexec(profile: *const libc::c_char) -> libc::c_int;
}
