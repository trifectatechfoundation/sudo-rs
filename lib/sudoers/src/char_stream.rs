pub trait CharStream {
    fn advance(&mut self);
    fn peek(&mut self) -> Option<char>;
    fn get_pos(&self) -> (usize, usize);
}

pub struct PeekableWithPos<Iter: Iterator> {
    iter: std::iter::Peekable<Iter>,
    line: usize,
    col: usize,
}

impl<Iter: Iterator<Item = char>> PeekableWithPos<Iter> {
    pub fn new(src: Iter) -> Self {
        PeekableWithPos {
            iter: src.peekable(),
            line: 1,
            col: 1,
        }
    }
}

impl<Iter: Iterator<Item = char>> CharStream for PeekableWithPos<Iter> {
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
impl<Iter: Iterator<Item = char>> CharStream for std::iter::Peekable<Iter> {
    fn advance(&mut self) {
        self.next();
    }

    fn peek(&mut self) -> Option<char> {
        self.peek().cloned()
    }

    fn get_pos(&self) -> (usize, usize) {
        (0, 0)
    }
}
