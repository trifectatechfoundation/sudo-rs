/// Match a test input with a pattern
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

#[allow(dead_code)]
pub(super) fn bracket_match(test: &[u8], pattern: &[u8]) -> bool {
    let mut match_cases: Vec<u8> = Vec::new();
    let mut pattern_index = 0;
    let mut is_negated = false;
    let mut last_dash = None;

    while let Some(p) = pattern.get(pattern_index) {
        if *p == b'[' || *p == b']' {
            pattern_index += 1;
        } else if *p == b'!' || *p == b'^' {
            pattern_index += 1;
            is_negated = true;
        } else if *p == b'-' {
            pattern_index += 1;
            last_dash = Some(pattern_index);
        } else if last_dash.is_some() {
            let last_push = match_cases.last().unwrap();
            for case in *last_push..=*p {
                match_cases.push(case);
            }
            last_dash = None;
            pattern_index += 1;
        } else {
            match_cases.push(*p);
            pattern_index += 1;
        }
    }

    if is_negated {
        !test.iter().any(|c| match_cases.contains(c))
    } else {
        test.iter().any(|c| match_cases.contains(c))
    }
}

#[cfg(test)]
mod tests {
    use super::bracket_match;
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

    #[test]
    fn test_bracket_match() {
        let tests = vec![
            ("foo", "[aeiou]", true),
            ("foo", "[xyz]", false),
            ("123", "[321]", true),
            ("123", "[456]", false),
            ("foo", "[xyz][fgh]", true),
            ("foo", "[AEIOU]", false),
            ("FOO", "[AEIOU]", true),
            ("foo", "[a-z]", true),
            ("foo", "[A-Z]", false),
            ("FOO", "[A-Z]", true),
            ("123", "[0-9]", true),
            ("foo", "[0-9]", false),
            ("foo", "[abc][123][e-j]", true),
            ("foo", "[^abc]", true),
            ("foo", "[!fgh]", false),
            ("foo", "[!a-c][!x-z]", true),
            ("123", "[^5-9]", true),
            ("foo bar", "[A-Za-z0-9]", true),
        ];

        for (test, pattern, expected) in tests.into_iter() {
            assert_eq!(
                bracket_match(test.as_bytes(), pattern.as_bytes()),
                expected,
                "\"{}\" {} match {}",
                test,
                if expected { "should" } else { "should not" },
                pattern
            );
        }
    }
}
