#![cfg_attr(not(test), allow(unused))]
use std::ffi::{CStr, CString};

/// If the locale isn't detected to be UTF-8, or couldn't be switched, the user
/// will get the default messages.
fn textdomain(domain: &CStr) {
    use libc::{nl_langinfo, setlocale, CODESET, LC_ALL};
    let utf8 = cstr!("UTF-8");

    // SAFETY: in all cases the functions are passed valid null-terminated C strings;
    // in the case of nl_langinfo, it is guaranteed by the spec to always return a valid
    // null-terminated C string as well, making the CStr::from_ptr call safe.
    unsafe {
        if setlocale(LC_ALL, CString::default().as_ptr()).is_null() {
            return;
        };
        if CStr::from_ptr(nl_langinfo(CODESET)) != utf8 {
            return;
        }
        if gettext_sys::bind_textdomain_codeset(domain.as_ptr(), utf8.as_ptr()).is_null() {
            return;
        }

        gettext_sys::textdomain(domain.as_ptr());
    }
}

fn gettext(text: &'static CStr) -> &'static str {
    // SAFETY: gettext() is guaranteed to return a pointer to a statically
    // allocated null-terminated string; this string is also constant (i.e.
    // it will be unmodified by future calls to gettext.)
    unsafe { CStr::from_ptr(gettext_sys::gettext(text.as_ptr())) }
        .to_str()
        .expect("translation files are corrupted")
}

macro_rules! xlat {
    ($text: literal) => {{
        debug_assert!(!$text.contains("{"), "invalid gettext input");
        gettext(cstr!($text))
    }};

    ($text: literal $(, $id: ident = $val: expr)*) => {{
        let fmt = gettext(cstr!($text));
        $(
        let fmt = fmt.replace(concat!("{", stringify!($id), "}"), $val.to_string().as_ref());
        )*

        debug_assert!(!fmt.contains("{"), "invalid gettext input");
        fmt
    }};
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn it_works() {
        textdomain(cstr!("sudo-rs"));
        let input = cstr!("sudo");
        // inputs that are not translated are not translated
        assert_eq!(gettext(input), input.to_str().unwrap());
        // .. in fact they are the same object
        assert_eq!(gettext(input).as_ptr(), input.to_str().unwrap().as_ptr());

        if std::env::var("LANG").unwrap_or_default().starts_with("nl") {
            assert_eq!(xlat!("usage"), "gebruik");
        }
    }

    #[test]
    fn var_subst() {
        assert_eq!(
            xlat!("{hello} {world}", world = "world", hello = "hello"),
            "hello world"
        );

        assert_eq!(xlat!("five = {five}", five = 5), "five = 5");
    }
}
