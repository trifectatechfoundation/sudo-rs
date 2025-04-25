use std::{ffi::CString, io::ErrorKind};

use crate::cutils::cerr;

mod sys;

pub fn apparmor_is_enabled() -> std::io::Result<bool> {
    match std::fs::read_to_string("/sys/module/apparmor/parameters/enabled") {
        Ok(enabled) => Ok(enabled.starts_with("Y")),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Switch the apparmor profile to the given profile on the next exec call
pub fn apparmor_prepare_exec(new_profile: &str) -> std::io::Result<()> {
    let new_profile_cstr = CString::new(new_profile)?;
    // SAFETY: new_profile_cstr provided by CString ensures a valid ptr
    cerr(unsafe { sys::aa_change_onexec(new_profile_cstr.as_ptr()) })?;

    Ok(())
}
