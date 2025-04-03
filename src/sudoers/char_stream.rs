pub struct CharStream<'a> {
    iter: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
    col: usize,
}

/// Advance the given position by `n` horizontal steps
pub fn advance(pos: (usize, usize), n: usize) -> (usize, usize) {
    (pos.0, pos.1 + n)
}

impl<'a> CharStream<'a> {
    pub fn new_with_pos(src: &'a str, (line, col): (usize, usize)) -> Self {
        CharStream {
            iter: src.chars().peekable(),
            line,
            col,
        }
    }

    pub fn new(src: &'a str) -> Self {
        Self::new_with_pos(src, (1, 1))
    }
}

impl CharStream<'_> {
    pub fn next_if(&mut self, f: impl FnOnce(char) -> bool) -> Option<char> {
        let item = self.iter.next_if(|&c| f(c));
        match item {
            Some('\n') => {
                self.line += 1;
                self.col = 1;
            }
            Some(_) => self.col += 1,
            _ => {}
        }
        item
    }

    pub fn eat_char(&mut self, expect_char: char) -> bool {
        self.next_if(|c| c == expect_char).is_some()
    }

    pub fn skip_to_newline(&mut self) {
        while self.next_if(|c| c != '\n').is_some() {}
    }

    pub fn peek(&mut self) -> Option<char> {
        self.iter.peek().cloned()
    }

    pub fn get_pos(&self) -> (usize, usize) {
        (self.line, self.col)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_iter() {
        let mut stream = CharStream::new("12\n3\n");
        assert_eq!(stream.peek(), Some('1'));
        assert!(stream.eat_char('1'));
        assert_eq!(stream.peek(), Some('2'));
        assert!(stream.eat_char('2'));
        assert!(stream.eat_char('\n'));
        assert_eq!(stream.peek(), Some('3'));
        assert!(stream.eat_char('3'));
        assert_eq!(stream.get_pos(), (2, 2));
    }
}
