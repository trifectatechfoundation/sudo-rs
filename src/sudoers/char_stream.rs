pub trait CharStream {
    fn advance(&mut self);
    fn peek(&mut self) -> Option<char>;
    fn get_pos(&self) -> (usize, usize);
}

pub struct PeekableWithPos<'a> {
    iter: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
    col: usize,
}

impl<'a> PeekableWithPos<'a> {
    pub fn new(src: std::str::Chars<'a>) -> Self {
        PeekableWithPos {
            iter: src.peekable(),
            line: 1,
            col: 1,
        }
    }
}

impl CharStream for PeekableWithPos<'_> {
    fn advance(&mut self) {
        match self.iter.next() {
            Some('\n') => {
                self.line += 1;
                self.col = 1;
            }
            Some(_) => self.col += 1,
            _ => {}
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.iter.peek().cloned()
    }

    fn get_pos(&self) -> (usize, usize) {
        (self.line, self.col)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_iter() {
        let mut stream = PeekableWithPos::new("12\n3\n".chars());
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
