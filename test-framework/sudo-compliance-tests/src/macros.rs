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
