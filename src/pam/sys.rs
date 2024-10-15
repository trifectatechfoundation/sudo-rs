/* automatically generated by rust-bindgen 0.70.1, minified by cargo-minify */

pub const PAM_SUCCESS: u32 = 0;
pub const PAM_OPEN_ERR: u32 = 1;
pub const PAM_SYMBOL_ERR: u32 = 2;
pub const PAM_SERVICE_ERR: u32 = 3;
pub const PAM_SYSTEM_ERR: u32 = 4;
pub const PAM_BUF_ERR: u32 = 5;
pub const PAM_PERM_DENIED: u32 = 6;
pub const PAM_AUTH_ERR: u32 = 7;
pub const PAM_CRED_INSUFFICIENT: u32 = 8;
pub const PAM_AUTHINFO_UNAVAIL: u32 = 9;
pub const PAM_USER_UNKNOWN: u32 = 10;
pub const PAM_MAXTRIES: u32 = 11;
pub const PAM_NEW_AUTHTOK_REQD: u32 = 12;
pub const PAM_ACCT_EXPIRED: u32 = 13;
pub const PAM_SESSION_ERR: u32 = 14;
pub const PAM_CRED_UNAVAIL: u32 = 15;
pub const PAM_CRED_EXPIRED: u32 = 16;
pub const PAM_CRED_ERR: u32 = 17;
pub const PAM_NO_MODULE_DATA: u32 = 18;
pub const PAM_CONV_ERR: u32 = 19;
pub const PAM_AUTHTOK_ERR: u32 = 20;
pub const PAM_AUTHTOK_RECOVERY_ERR: u32 = 21;
pub const PAM_AUTHTOK_LOCK_BUSY: u32 = 22;
pub const PAM_AUTHTOK_DISABLE_AGING: u32 = 23;
pub const PAM_TRY_AGAIN: u32 = 24;
pub const PAM_IGNORE: u32 = 25;
pub const PAM_ABORT: u32 = 26;
pub const PAM_AUTHTOK_EXPIRED: u32 = 27;
pub const PAM_MODULE_UNKNOWN: u32 = 28;
pub const PAM_BAD_ITEM: u32 = 29;
pub const PAM_SILENT: u32 = 32768;
pub const PAM_DISALLOW_NULL_AUTHTOK: u32 = 1;
pub const PAM_REINITIALIZE_CRED: u32 = 8;
pub const PAM_CHANGE_EXPIRED_AUTHTOK: u32 = 32;
pub const PAM_USER: u32 = 2;
pub const PAM_TTY: u32 = 3;
pub const PAM_RUSER: u32 = 8;
pub const PAM_DATA_SILENT: u32 = 1073741824;
pub const PAM_PROMPT_ECHO_OFF: u32 = 1;
pub const PAM_PROMPT_ECHO_ON: u32 = 2;
pub const PAM_ERROR_MSG: u32 = 3;
pub const PAM_TEXT_INFO: u32 = 4;
pub const PAM_MAX_RESP_SIZE: u32 = 512;
pub type pam_handle_t = u8;
extern "C" {
    pub fn pam_set_item(
        pamh: *mut pam_handle_t,
        item_type: libc::c_int,
        item: *const libc::c_void,
    ) -> libc::c_int;
}
extern "C" {
    pub fn pam_get_item(
        pamh: *const pam_handle_t,
        item_type: libc::c_int,
        item: *mut *const libc::c_void,
    ) -> libc::c_int;
}
extern "C" {
    pub fn pam_strerror(pamh: *mut pam_handle_t, errnum: libc::c_int) -> *const libc::c_char;
}
extern "C" {
    pub fn pam_getenvlist(pamh: *mut pam_handle_t) -> *mut *mut libc::c_char;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct pam_message {
    pub msg_style: libc::c_int,
    pub msg: *const libc::c_char,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct pam_response {
    pub resp: *mut libc::c_char,
    pub resp_retcode: libc::c_int,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct pam_conv {
    pub conv: ::std::option::Option<
        unsafe extern "C" fn(
            num_msg: libc::c_int,
            msg: *mut *const pam_message,
            resp: *mut *mut pam_response,
            appdata_ptr: *mut libc::c_void,
        ) -> libc::c_int,
    >,
    pub appdata_ptr: *mut libc::c_void,
}
extern "C" {
    pub fn pam_start(
        service_name: *const libc::c_char,
        user: *const libc::c_char,
        pam_conversation: *const pam_conv,
        pamh: *mut *mut pam_handle_t,
    ) -> libc::c_int;
}
extern "C" {
    pub fn pam_end(pamh: *mut pam_handle_t, pam_status: libc::c_int) -> libc::c_int;
}
extern "C" {
    pub fn pam_authenticate(pamh: *mut pam_handle_t, flags: libc::c_int) -> libc::c_int;
}
extern "C" {
    pub fn pam_setcred(pamh: *mut pam_handle_t, flags: libc::c_int) -> libc::c_int;
}
extern "C" {
    pub fn pam_acct_mgmt(pamh: *mut pam_handle_t, flags: libc::c_int) -> libc::c_int;
}
extern "C" {
    pub fn pam_open_session(pamh: *mut pam_handle_t, flags: libc::c_int) -> libc::c_int;
}
extern "C" {
    pub fn pam_close_session(pamh: *mut pam_handle_t, flags: libc::c_int) -> libc::c_int;
}
extern "C" {
    pub fn pam_chauthtok(pamh: *mut pam_handle_t, flags: libc::c_int) -> libc::c_int;
}
pub type __uid_t = libc::c_uint;
pub type __gid_t = libc::c_uint;
