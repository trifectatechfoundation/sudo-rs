//! Macros useful in tests

/// assert_contains(x,y) is equivalent to assert!(x.contains(y))
#[macro_export]
macro_rules! assert_contains {
    ($haystack:expr, $needle:expr) => {
        let haystack = &$haystack;
        let needle = &$needle;

        assert!(
            haystack.contains(needle),
            "{haystack:?} did not contain {needle:?}"
        )
    };
}

/// assert_contains(x,y) is equivalent to !assert!(x.contains(y))
#[macro_export]
macro_rules! assert_not_contains {
    ($haystack:expr, $needle:expr) => {
        let haystack = &$haystack;
        let needle = &$needle;

        assert!(
            !haystack.contains(needle),
            "{haystack:?} did contain {needle:?}"
        )
    };
}

/// assert_starts_with(x,y) is equivalent to assert!(x.starts_with(y))
#[macro_export]
macro_rules! assert_starts_with {
    ($haystack:expr, $needle:expr) => {
        let haystack = &$haystack;
        let needle = &$needle;

        assert!(
            haystack.starts_with(needle),
            "{haystack:?} did not start with {needle:?}"
        )
    };
}
