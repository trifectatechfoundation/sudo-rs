use std::ffi::CStr;

use std::mem::zeroed;

use crate::common::Error;

pub fn kernel_check(major: u32, minor: u32) -> Result<(), Error> {
    let mut utsname: libc::utsname = unsafe { zeroed() };

    if unsafe { libc::uname(&mut utsname) } != 0 {
        // Could not get the kernel version. Try to run anyway
        return Ok(());
    }

    let release = unsafe { CStr::from_ptr(utsname.release.as_ptr()) }
        .to_string_lossy()
        .into_owned();

    let version_parts: Vec<&str> = release.split('.').collect();

    if version_parts.len() < 2 {
        // Could not get the kernel version. Try to run anyway
        return Ok(());
    }

    // Parse the major and minor version numbers
    if let (Ok(major_version), Ok(minor_version)) = (
        version_parts[0].parse::<u32>(),
        version_parts[1].parse::<u32>(),
    ) {
        if major_version > major || (major_version == major && minor_version >= minor) {
            return Ok(());
        }
    }

    Err(Error::KernelCheck)
}
