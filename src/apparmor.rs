use std::{ffi::CString, io::ErrorKind};

use crate::cutils::cerr;

/// Set the profile for the next exec call if AppArmor is enabled
pub fn set_profile_for_next_exec(profile_name: &str) -> std::io::Result<()> {
    if apparmor_is_enabled()? {
        apparmor_prepare_exec(profile_name)
    } else {
        // if the sysadmin doesn't have apparmor enabled, fail softly
        Ok(())
    }
}

fn apparmor_is_enabled() -> std::io::Result<bool> {
    match std::fs::read_to_string("/sys/module/apparmor/parameters/enabled") {
        Ok(enabled) => Ok(enabled.starts_with("Y")),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

#[link(name = "apparmor")]
extern "C" {
    pub fn aa_change_onexec(profile: *const libc::c_char) -> libc::c_int;
}

/// Switch the apparmor profile to the given profile on the next exec call
fn apparmor_prepare_exec(new_profile: &str) -> std::io::Result<()> {
    let new_profile_cstr = CString::new(new_profile)?;
    // SAFETY: new_profile_cstr provided by CString ensures a valid ptr
    cerr(unsafe { aa_change_onexec(new_profile_cstr.as_ptr()) })?;

    Ok(())
}
