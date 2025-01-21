pub struct CharStream<'a> {
    iter: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
    col: usize,
}

impl<'a> CharStream<'a> {
    pub fn new(src: std::str::Chars<'a>) -> Self {
        CharStream {
            iter: src.peekable(),
            line: 1,
            col: 1,
        }
    }
}

impl CharStream<'_> {
    pub fn advance(&mut self) {
        match self.iter.next() {
            Some('\n') => {
                self.line += 1;
                self.col = 1;
            }
            Some(_) => self.col += 1,
            _ => {}
        }
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
        let mut stream = CharStream::new("12\n3\n".chars());
        assert_eq!(stream.peek(), Some('1'));
        stream.advance();
        assert_eq!(stream.peek(), Some('2'));
        stream.advance();
        stream.advance();
        assert_eq!(stream.peek(), Some('3'));
        stream.advance();
        assert_eq!(stream.get_pos(), (2, 2));
    }
}
