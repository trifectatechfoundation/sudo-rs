use std::ffi::{c_char, c_int, CStr, CString};
use std::{fs, io, mem};

use crate::cutils::cerr;

/// Set the profile for the next exec call if AppArmor is enabled
pub fn set_profile_for_next_exec(profile_name: &str) -> io::Result<()> {
    if apparmor_is_enabled()? {
        apparmor_prepare_exec(profile_name)
    } else {
        // if the sysadmin doesn't have apparmor enabled, fail softly
        Ok(())
    }
}

fn apparmor_is_enabled() -> io::Result<bool> {
    match fs::read_to_string("/sys/module/apparmor/parameters/enabled") {
        Ok(enabled) => Ok(enabled.starts_with("Y")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// Switch the apparmor profile to the given profile on the next exec call
fn apparmor_prepare_exec(new_profile: &str) -> io::Result<()> {
    // SAFETY: Always safe to call
    unsafe { libc::dlerror() }; // Clear any existing error

    // SAFETY: Loading a known safe dylib. LD_LIBRARY_PATH is ignored because we are setuid.
    let handle = unsafe { libc::dlopen(c"libapparmor.so.1".as_ptr(), libc::RTLD_NOW) };
    if handle.is_null() {
        // SAFETY: In case of an error, dlerror returns a valid C string.
        return Err(io::Error::new(io::ErrorKind::NotFound, unsafe {
            CStr::from_ptr(libc::dlerror())
                .to_string_lossy()
                .into_owned()
        }));
    }

    // SAFETY: dlsym will either return a function pointer of the right signature or NULL.
    let aa_change_onexec = unsafe { libc::dlsym(handle, c"aa_change_onexec".as_ptr()) };

    if aa_change_onexec.is_null() {
        // SAFETY: Always safe to call
        let err = unsafe { libc::dlerror() };
        return Err(if err.is_null() {
            // There was no error in dlsym, but the symbol itself was defined as NULL pointer.
            // This is still an error for us, but dlerror will not return any error.
            io::Error::new(
                io::ErrorKind::Other,
                "aa_change_onexec symbol is a NULL pointer",
            )
        } else {
            // SAFETY: In case of an error, dlerror returns a valid C string.
            io::Error::new(io::ErrorKind::NotFound, unsafe {
                CStr::from_ptr(err).to_string_lossy().into_owned()
            })
        });
    }

    //SAFETY: aa_change_onexec is non-NULL, so we can cast it into a function pointer
    let aa_change_onexec: unsafe extern "C" fn(*const c_char) -> c_int =
        unsafe { mem::transmute(aa_change_onexec) };

    let new_profile_cstr = CString::new(new_profile)?;
    // SAFETY: new_profile_cstr provided by CString ensures a valid ptr
    cerr(unsafe { aa_change_onexec(new_profile_cstr.as_ptr()) })?;

    Ok(())
}
