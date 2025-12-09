#[cfg(feature = "gettext")]
use std::ffi::{CStr, CString};

/// If the locale isn't detected to be UTF-8, or couldn't be switched, the user
/// will get the default messages.
#[cfg(feature = "gettext")]
pub(crate) fn textdomain(domain: &CStr) {
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

#[cfg(feature = "gettext")]
pub(crate) mod display {
    pub trait DisplayStr {
        fn display(&self) -> impl AsRef<str>;
    }

    impl DisplayStr for &str {
        fn display(&self) -> impl AsRef<str> {
            self
        }
    }

    impl DisplayStr for String {
        fn display(&self) -> impl AsRef<str> {
            self
        }
    }

    impl<T: std::fmt::Display> DisplayStr for &T {
        fn display(&self) -> impl AsRef<str> {
            self.to_string()
        }
    }
}

#[cfg(feature = "gettext")]
pub(crate) fn gettext(text: &'static CStr) -> &'static str {
    // SAFETY: gettext() is guaranteed to return a pointer to a statically
    // allocated null-terminated string; this string is also constant (i.e.
    // it will be unmodified by future calls to gettext.)
    unsafe { CStr::from_ptr(gettext_sys::gettext(text.as_ptr())) }
        .to_str()
        .expect("translation files are corrupted")
}

#[cfg(feature = "gettext")]
macro_rules! xlat {
    ($text: literal) => {{
        debug_assert!(!$text.contains("{"), "invalid gettext input");
        $crate::gettext::gettext(cstr!($text))
    }};

    ($text: literal $(, $id: ident = $val: expr)*) => {{
        use $crate::gettext::display::DisplayStr;
        let fmt = $crate::gettext::gettext(cstr!($text));
        $(
        let fmt = fmt.replace(concat!("{", stringify!($id), "}"), DisplayStr::display(&&$val).as_ref());
        )*

        debug_assert!(!fmt.contains("{"), "invalid gettext input");
        fmt
    }};
}

#[cfg(not(feature = "gettext"))]
macro_rules! xlat {
    ($text: literal) => { $text };

    ($text: literal $(, $id: ident = $val: expr)*) => {{
        format!($text $(,$id = $val)*)
    }};
}

#[cfg(feature = "gettext")]
macro_rules! xlat_write {
    ($f: expr, $fmt: literal $(, $id: ident = $val: expr)*) => {
        write!($f, "{}", $crate::gettext::xlat!($fmt $(, $id = $val)*))
    };
}

#[cfg(not(feature = "gettext"))]
macro_rules! xlat_write {
    ($f: expr, $fmt: literal $(, $id: ident = $val: expr)*) => {
        write!($f, $fmt $(, $id = $val)*)
    };
}

pub(crate) use xlat;
pub(crate) use xlat_write;

//These are all defined in POSIX.
#[cfg(feature = "gettext")]
mod gettext_sys {
    extern "C" {
        pub fn gettext(msgid: *const libc::c_char) -> *mut libc::c_char;

        pub fn textdomain(domain: *const libc::c_char) -> *mut libc::c_char;

        pub fn bind_textdomain_codeset(
            domain: *const libc::c_char,
            codeset: *const libc::c_char,
        ) -> *mut libc::c_char;
    }
}

#[cfg(test)]
mod test {
    #[test]
    #[cfg(feature = "gettext")]
    fn it_works() {
        use super::*;
        textdomain(cstr!("sudo-rs"));
        let input = cstr!("sudo");
        // inputs that are not translated are not translated
        assert_eq!(gettext(input), input.to_str().unwrap());
        // .. in fact they are the same object
        assert_eq!(gettext(input).as_ptr(), input.to_str().unwrap().as_ptr());

        #[cfg(feature = "gettext")]
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

    #[test]
    #[cfg(feature = "gettext")]
    fn str_optimized() {
        use super::display::DisplayStr;
        let foo: &str = "foo";
        assert_eq!(foo.display().as_ref().as_ptr(), foo.as_ptr());
        let foo: &&str = &"foo";
        assert_eq!(foo.display().as_ref().as_ptr(), foo.as_ptr());
        let foo: String = "foo".to_string();
        assert_eq!(foo.display().as_ref().as_ptr(), foo.as_ptr());
        let foo: &String = &foo;
        assert_eq!(foo.display().as_ref().as_ptr(), foo.as_ptr());
    }
}
