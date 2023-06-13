/// Match a  test input with a pattern
/// Only wildcard characters (*) in the pattern string have a special meaning: they match on zero or more characters
pub(super) fn wildcard_match(test: &[u8], pattern: &[u8]) -> bool {
    let mut test_index = 0;
    let mut pattern_index = 0;
    let mut last_star = None;

    loop {
        match (pattern.get(pattern_index), test.get(test_index)) {
            (Some(p), Some(t)) => {
                if *p == b'*' {
                    pattern_index += 1;
                    last_star = Some((test_index, pattern_index));
                } else if p == t {
                    pattern_index += 1;
                    test_index += 1;
                } else if let Some((t_index, p_index)) = last_star {
                    test_index = t_index + 1;
                    pattern_index = p_index;
                    last_star = Some((test_index, pattern_index));
                } else {
                    return false;
                }
            }
            (None, Some(_)) => {
                if let Some((t_index, p_index)) = last_star {
                    test_index = t_index + 1;
                    pattern_index = p_index;
                    last_star = Some((test_index, pattern_index));
                } else {
                    return false;
                }
            }
            (Some(b'*'), None) => {
                pattern_index += 1;
            }
            (None, None) => {
                return true;
            }
            _ => {
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::wildcard_match;

    #[test]
    fn test_wildcard_match() {
        let tests = vec![
            ("foo bar", "foo *", true),
            ("foo bar", "foo ba*", true),
            ("foo bar", "foo *ar", true),
            ("foo bar", "foo *r", true),
            ("foo bar", "foo *ab", false),
            ("foo bar", "foo r*", false),
            ("foo bar", "*oo bar", true),
            ("foo bar", "*f* bar", true),
            ("foo bar", "*f bar", false),
            ("foo ", "foo *", true),
            ("foo", "foo *", false),
            ("foo", "foo*", true),
            ("foo bar", "f*******r", true),
            ("foo******bar", "f*r", true),
            ("foo********bar", "foo bar", false),
            ("#%^$V@#TYH%&rot13%#@$%#$%", "#%^$V@#*t13%#@$%#$%", true),
            ("#%^$V@#TYH%&rot13%#@$%#$%", "*%^*%&rot*%#$%", true),
            ("#%^$V@#TYH%&rot13%#@$%#$%", "#%^$V@#TYH%&r*%#@$#$%", false),
            ("#%^$V@#TYH%&rot13%#@$%#$%", "#%^$V@#*******@$%#$%", true),
        ];

        for (test, pattern, expected) in tests.into_iter() {
            assert_eq!(
                wildcard_match(test.as_bytes(), pattern.as_bytes()),
                expected,
                "\"{}\" {} match {}",
                test,
                if expected { "should" } else { "should not" },
                pattern
            );
        }
    }
}
