use crate::common::Error;

#[cfg(target_os = "linux")]
pub fn kernel_check() -> Result<(), Error> {
    use crate::cutils::cerr;
    use std::ffi::CStr;
    use std::mem::MaybeUninit;

    // On Linux, we need kernel version 5.9 to have access to `close_range()`
    const TARGET_VERSION: (u32, u32) = (5, 9);

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
        (Some(major), Some(minor)) if (major, minor) < TARGET_VERSION => {
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

#[cfg(any(target_os = "freebsd", target_os = "macos"))]
pub fn kernel_check() -> Result<(), Error> {
    // the kernel check doesn't make much sense on FreeBSD (we need FreeBSD 8.0 or newer,
    // which is comparatively ancient compared to Linux 5.9)
    Ok(())
}

#[cfg(not(any(target_os = "freebsd", target_os = "linux", target_os = "macos")))]
pub fn kernel_check() -> Result<(), Error> {
    compile_error!("sudo-rs only works on Linux, FreeBSD and MacOS")
}
