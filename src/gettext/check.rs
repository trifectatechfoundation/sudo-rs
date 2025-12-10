const fn in_set(pat: &[u8], range: std::ops::Range<usize>, keys: &[&str]) -> bool {
    let mut k = 0;
    while k < keys.len() {
        'keycheck: {
            if range.end - range.start == keys[k].len() {
                let mut i = range.start;
                while i < range.end {
                    if pat[i] != keys[k].as_bytes()[i - range.start] {
                        break 'keycheck;
                    }
                    i += 1
                }

                return true;
            }
        }
        k += 1
    }

    false
}

pub const fn check_keys(pat: &str, keys: &[&str]) {
    let pat = pat.as_bytes();
    let mut i = 0;
    'outer: while i < pat.len() {
        if pat[i] == b'{' {
            let mut j = i + 1;
            while j < pat.len() {
                if pat[j] == b'}' {
                    assert!(
                        in_set(pat, i + 1..j, keys),
                        "unmatched key in xlat-format string"
                    );
                    i = j + 1;
                    continue 'outer;
                }
                j += 1
            }

            panic!("unmatched }} in pattern");
        }

        i += 1
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    const fn eq_works() {
        assert!(in_set(b"flerbage", 0..8, &["flerbage"]));
        assert!(!in_set(b"foo", 0..3, &["flerbage"]));
        assert!(!in_set(b"flerbage", 0..8, &["foo"]));
        assert!(!in_set(b"flarbege", 0..8, &["flerbage"]));
        assert!(in_set(b"flerbage", 0..8, &["foo", "flerbage"]));
        assert!(in_set(b"flerbage", 0..8, &["flerbage", "foo"]));
        assert!(!in_set(b"bar", 0..3, &["flerbage", "foo"]));
        assert!(!in_set(b"flerbage", 0..7, &["flerbage"]));
        assert!(in_set(b"_flerbage_", 1..9, &["flerbage"]));
    }

    #[test]
    fn check_works() {
        check_keys("{foo}{bar}", &["bar", "foo"]);
    }

    #[should_panic]
    #[test]
    fn check_panics() {
        check_keys("{foo}", &["bar"]);
    }
}
