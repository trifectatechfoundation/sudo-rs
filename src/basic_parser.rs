use std::iter::Peekable;

/* Contract:
 *
 * if the parse method of this trait returns None, the iterator is not advanced; otherwise it is
 * advanced beyond the accepted part of the input. i.e. if some input is consumed the method
 * should be producing some value.
 */

pub trait Parse {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self>
    where
        Self: Sized;
}

// primitive function
pub fn accept_if(
    predicate: impl Fn(char) -> bool,
    stream: &mut Peekable<impl Iterator<Item = char>>,
) -> Option<char> {
    let &c = stream.peek()?;
    if predicate(c) {
        stream.next();
        Some(c)
    } else {
        None
    }
}

#[derive(Debug)]
struct Whitespace;

impl Parse for Whitespace {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let mut eat_space = || accept_if(char::is_whitespace, stream);
        eat_space()?;
        while eat_space().is_some() {}
        Some(Whitespace {})
    }
}

// same as accept_if, but parses whitespace
pub fn is_syntax(syntax: char, stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<()> {
    accept_if(|c| c == syntax, stream)?;
    Whitespace::parse(stream);
    Some(())
}

pub fn expect_syntax(syntax: char, stream: &mut Peekable<impl Iterator<Item = char>>) {
    if is_syntax(syntax, stream).is_none() {
        let str = if let Some(c) = stream.peek() {
            c.to_string()
        } else {
            "EOL".to_string()
        };
        panic!("parse error: expecting `{}' but found `{}'", syntax, str)
    }
}

pub fn is_some<T: Parse>(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<T> {
    T::parse(stream)
}

pub fn expect_some<T: Parse>(stream: &mut Peekable<impl Iterator<Item = char>>) -> T {
    let Some(result) = is_some(stream) else {
        panic!("parse error: expected `{}'", std::any::type_name::<T>())
    };
    result
}

// implement a single parse method for "tokens" (defined in tokens.rs)
pub trait Token {
    const IDENT: fn(String) -> Self;
    const MAX_LEN: usize = 255;

    fn accept(c: char) -> bool;
    fn accept_1st(c: char) -> bool {
        Self::accept(c)
    }

    const ESCAPE: char = '\0';
    fn escaped(_: char) -> bool {
        false
    }
}

impl<T: Token> Parse for T {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let mut str = accept_if(T::accept_1st, stream)?.to_string();
        loop {
            if let Some(c) = accept_if(T::accept, stream) {
                str.push(c)
            } else if accept_if(|c| c == T::ESCAPE, stream).is_some() {
                if let Some(c) = accept_if(T::escaped, stream) {
                    str.push(c)
                } else {
                    panic!("tokenizer: illegal escape sequence")
                }
            } else {
                break;
            }
            if str.len() >= T::MAX_LEN {
                panic!("tokenizer: exceeded safety margin")
            }
        }
        Whitespace::parse(stream);
        Some(T::IDENT(str))
    }
}

// I would recommend not using this for anything that has more than two alternatives
impl<T1: Token, T2: Parse> Parse for Result<T1, T2> {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let &c = stream.peek()?;
        if T1::accept(c) {
            T1::parse(stream).map(Ok)
        } else {
            T2::parse(stream).map(Err)
        }
    }
}

fn parse_list<T: Parse>(
    sep_by: char,
    max: usize,
    stream: &mut Peekable<impl Iterator<Item = char>>,
) -> Option<Vec<T>> {
    let mut elems = Vec::new();
    elems.push(is_some(stream)?);
    while is_syntax(sep_by, stream).is_some() {
        if elems.len() >= max {
            panic!("parse_list: parsing multiple items: safety margin exceeded")
        }
        elems.push(expect_some(stream));
    }
    Some(elems)
}

// A trait that specified parsed elements can be repeated; enabling a Vec<T> parser
pub trait Many {
    const SEP: char = ',';
    const LIMIT: usize = 127;
}

impl<T: Parse + Many> Parse for Vec<T> {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        parse_list(T::SEP, T::LIMIT, stream)
    }
}

#[allow(dead_code)]
fn expect_end_of_parse(stream: &mut Peekable<impl Iterator<Item = char>>) {
    if stream.peek().is_some() {
        panic!("parse error: trailing garbage")
    }
}

#[allow(dead_code)]
pub fn is_complete<T: Parse>(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<T> {
    let result = is_some(stream)?;
    expect_end_of_parse(stream);
    Some(result)
}

#[allow(dead_code)]
pub fn expect_complete<T: Parse>(stream: &mut Peekable<impl Iterator<Item = char>>) -> T {
    let result = expect_some(stream);
    expect_end_of_parse(stream);
    result
}
