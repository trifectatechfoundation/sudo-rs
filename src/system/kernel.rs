use std::ffi::CStr;

use std::mem::MaybeUninit;

use crate::{common::Error, cutils::cerr};

#[cfg(target_os = "linux")]
pub fn kernel_check(target_major: u32, target_minor: u32) -> Result<(), Error> {
    let mut utsname = MaybeUninit::uninit();

    // SAFETY: uname is passed a correct pointer
    cerr(unsafe { libc::uname(utsname.as_mut_ptr()) })?;

    // SAFETY: since uname exited normally, the struct is now initialized
    let utsname = unsafe { utsname.assume_init() };

    // SAFETY: utsname.release will hold a null-terminated C string
    let release = unsafe { CStr::from_ptr(utsname.release.as_ptr()) }.to_string_lossy();

    // Parse the major and minor version numbers
    let mut version_parts = release.split('.').map_while(|x| x.parse::<u32>().ok());

    match (version_parts.next(), version_parts.next()) {
        (Some(major), Some(minor)) if (major, minor) < (target_major, target_minor) => {
            // We have determined that this Linux kernel is too old.
            Err(Error::KernelCheck)
        }
        _ => {
            // We have not been able to prove that sudo-rs is incompatible with this kernel
            // and are giving the benefit of the doubt.
            Ok(())
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn kernel_check(target_major: u32, target_minor: u32) -> Result<(), Error> {
    // if someone managed to compile this on anything else than Linux: your luck runs out here
    Err(Error::KernelCheck)
}
