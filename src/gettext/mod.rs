#[cfg(feature = "gettext")]
use std::{
    ffi::{CStr, CString},
    sync::OnceLock,
};

#[cfg(feature = "gettext")]
pub(crate) mod check;

#[cfg(feature = "gettext")]
// If the locale isn't detected to be UTF-8, or couldn't be switched, the user
// will get the default messages.
static TEXT_DOMAIN: OnceLock<&'static CStr> = OnceLock::new();

#[cfg(feature = "gettext")]
pub(crate) fn textdomain(domain: &'static CStr) {
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
    }

    TEXT_DOMAIN.set(domain).expect("only set the locale once")
}

#[cfg(feature = "gettext")]
pub(crate) mod display {
    // Based on <https://lukaskalbertodt.github.io/2019/12/05/generalized-autoref-based-specialization.html>
    pub struct Wrap<T: ?Sized>(pub T);

    pub trait Convert {
        fn display(&self) -> String;
    }

    pub trait Reference {
        fn display(&self) -> &str;
    }

    impl<T: std::fmt::Display + ?Sized> Convert for Wrap<T> {
        fn display(&self) -> String {
            self.0.to_string()
        }
    }

    impl<T: std::fmt::Display + AsRef<str> + ?Sized> Reference for &Wrap<T> {
        fn display(&self) -> &str {
            self.0.as_ref()
        }
    }
}

#[cfg(feature = "gettext")]
pub(crate) fn gettext(text: &'static CStr) -> &'static str {
    // SAFETY:
    // - dggettext expects its first argument to be NULL or a pointer to a
    // valid C string; its second argument should always be a valid C string
    // - dgettext() is guaranteed to return a pointer to a statically
    // allocated null-terminated string; this string is also constant (i.e.
    // it will be unmodified by future calls to gettext.)
    unsafe {
        CStr::from_ptr(gettext_sys::dgettext(
            TEXT_DOMAIN
                .get()
                .map_or(std::ptr::null(), |domain| domain.as_ptr()),
            text.as_ptr(),
        ))
    }
    .to_str()
    .expect("translation files are corrupted")
}

#[cfg(feature = "gettext")]
macro_rules! xlat {
    ($text: literal) => {{
        #[allow(dead_code)]
        const _OK: () = $crate::gettext::check::check_keys($text, &[]);
        $crate::gettext::gettext(cstr!($text))
    }};

    ($text: literal $(, $id: ident = $val: expr)* $(,)?) => {{
        #[allow(unused)]
        use $crate::gettext::display::{Convert, Reference, Wrap};
        use std::ops::Deref;

        #[allow(dead_code)]
        const _OK: () = $crate::gettext::check::check_keys(
            $text,
            &[$(stringify!($id)),*]
        );

        let result = $crate::gettext::gettext(cstr!($text));
        $(
        let result = result.replace(
            concat!("{", stringify!($id), "}"),
            (&&Wrap(&$val)).display().deref(),
        );
        )*

        result
    }};
}

#[cfg(not(feature = "gettext"))]
macro_rules! xlat {
    ($text: literal) => { $text };

    ($text: literal $(, $id: ident = $val: expr)* $(,)?) => {{
        format!($text $(,$id = $val)*)
    }};
}

#[cfg(feature = "gettext")]
macro_rules! xlat_write {
    ($f: expr, $fmt: literal $(, $id: ident = $val: expr)* $(,)?) => {
        write!($f, "{}", xlat!($fmt $(, $id = $val)*))
    };
}

#[cfg(feature = "gettext")]
macro_rules! xlat_println {
    ($fmt: literal $(, $id: ident = $val: expr)* $(,)?) => {
        println_ignore_io_error!("{}", xlat!($fmt $(, $id = $val)*))
    };
}

#[cfg(not(feature = "gettext"))]
macro_rules! xlat_write {
    ($f: expr, $fmt: literal $(, $id: ident = $val: expr)* $(,)?) => {
        write!($f, $fmt $(, $id = $val)*)
    };
}

#[cfg(not(feature = "gettext"))]
macro_rules! xlat_println {
    ($fmt: literal $(, $id: ident = $val: expr)* $(,)?) => {
        println_ignore_io_error!($fmt $(, $id = $val)*)
    };
}

//These are all defined in POSIX.
#[cfg(feature = "gettext")]
mod gettext_sys {
    #[cfg_attr(target_os = "freebsd", link(name = "intl"))]
    extern "C" {
        pub fn dgettext(
            domain: *const libc::c_char,
            msgid: *const libc::c_char,
        ) -> *mut libc::c_char;

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
        textdomain(cstr!("libc"));
        let input = cstr!("Hello World");
        // inputs that are not translated are not translated
        assert_eq!(gettext(input), input.to_str().unwrap());
        // .. in fact they are the same object
        assert_eq!(gettext(input).as_ptr(), input.to_str().unwrap().as_ptr());

        if std::env::var("LANG").unwrap_or_default().starts_with("nl") {
            assert_eq!(xlat!("Operation not permitted"), "Actie is niet toegestaan");
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
        use super::display::{Reference, Wrap};

        // in principle the assert_eq's below could be replaced by "expect(unused)" on this trait
        #[allow(unused_imports)]
        use super::display::Convert;

        let foo: &str = "foo";
        let addr = foo.as_ptr();
        assert_eq!((&&Wrap(&foo)).display().as_ptr(), addr);
        assert_eq!((&&Wrap(foo)).display().as_ptr(), addr);

        let foo: String = "foo".to_string();
        let addr = foo.as_ptr();
        assert_eq!((&&Wrap(&foo)).display().as_ptr(), addr);
        assert_eq!((&&Wrap(foo)).display().as_ptr(), addr);

        let foo: Box<str> = "foo".to_string().into_boxed_str();
        let addr = foo.as_ptr();
        assert_eq!((&&Wrap(&foo)).display().as_ptr(), addr);
        assert_eq!((&&Wrap(foo)).display().as_ptr(), addr);

        use crate::common::SudoString;
        let foo: SudoString = SudoString::new("foo".to_string()).unwrap();
        let addr = foo.as_str().as_ptr();
        assert_eq!((&&Wrap(&foo)).display().as_ptr(), addr);
        assert_eq!((&&Wrap(foo)).display().as_ptr(), addr);
    }
}
