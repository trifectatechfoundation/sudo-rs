//! Building blocks for a recursive descent LL(1) parsing method.
//!
//! The general idea is that a grammar (without left recursion) is translated to a series of
//! conditional and unconditional 'acceptance' methods.
//!
//! For example, assuming we have a parser for integers:
//!
//! sum = integer | integer + sum
//!
//! Can get translated as: (representing a sum as `LinkedList<u32>`):
//!
//! ```ignore
//! impl<T: Parse> Parse for LinkedList<T> {
//!     fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<LinkedList<T>> {
//!         let x = try_nonterminal(stream)?;
//!         let mut tail = if maybe(try_syntax('+', stream))?.is_some() {
//!             expect_nonterminal(stream)?
//!         } else {
//!             LinkedList::new()
//!         };
//!         tail.push_front(x);
//!
//!         make(tail)
//!     }
//! }
//! ```

use std::iter::Peekable;

/// Type holding a parsed object (or error information if parsing failed)
pub type Parsed<T> = Result<T, Status>;

#[derive(Debug, Clone)]
pub enum Status {
    Fatal(String), // not recoverable; stream in inconsistent state
    Reject,        // parsing failed by no input consumed
}

pub fn make<T>(value: T) -> Parsed<T> {
    Ok(value)
}

pub fn reject<T>() -> Parsed<T> {
    Err(Status::Reject)
}

macro_rules! unrecoverable {
    ($($str:expr),*) => {
        return Err(crate::basic_parser::Status::Fatal(format![$($str),*]))
    }
}

pub(crate) use unrecoverable;

/// This recovers from a failed parsing.
pub fn maybe<T>(status: Parsed<T>) -> Parsed<Option<T>> {
    match status {
        Ok(x) => Ok(Some(x)),
        Err(Status::Reject) => Ok(None),
        Err(err) => Err(err),
    }
}

/// This turns recoverable errors into non-recoverable ones.
pub fn force<T>(status: Parsed<T>) -> Parsed<T> {
    match status {
        Err(Status::Reject) => {
            unrecoverable!("parse error: expected `{}'", std::any::type_name::<T>())
        }
        _ => status,
    }
}

/// All implementations of the Parse trait must satisfy this contract:
///
/// If the `parse` method of this trait returns None, the iterator is not advanced; otherwise it is
/// advanced beyond the accepted part of the input. i.e. if some input is consumed the method
/// *MUST* be producing a `Some` value.
pub trait Parse {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self>
    where
        Self: Sized;
}

/// Primitive function (that also adheres to the Parse trait contract): accepts one character
/// that satisfies `predicate`. This is used in the majority of all the other `Parse`
/// implementations instead of interfacing with the iterator directly (this can facilitate an easy
/// switch to a different method of stream representation in the future). Unlike most `Parse`
/// implementations this *does not* consume trailing whitespace.
/// NOTE: Guaranteed not to give an unrecoverable error.
pub fn accept_if(
    predicate: impl Fn(char) -> bool,
    stream: &mut Peekable<impl Iterator<Item = char>>,
) -> Parsed<char> {
    let &c = stream.peek().ok_or(Status::Reject)?;
    if predicate(c) {
        stream.next();
        make(c)
    } else {
        reject()
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]

/// A structure representing whitespace
struct Whitespace;

/// Accept one or more whitespace characters; fails if no whitespace is found (to parse zero or
/// more whitespace characters, parse `Option<Whitespace>`
impl Parse for Whitespace {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        let eat_space = |stream: &mut _| accept_if(|c| "\t ".contains(c), stream);

        let comment = |stream: &mut _| {
            accept_if(|c| c == '#', stream)?;
            while accept_if(|c| c != '\n', stream).is_ok() {}
            make(())
        };

        if eat_space(stream).is_ok() {
            while eat_space(stream).is_ok() {}
            maybe(comment(stream))?;
        } else {
            comment(stream)?;
        }

        make(Whitespace {})
    }
}

pub fn skip_whitespace(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<()> {
    maybe(Whitespace::parse(stream))?;
    make(())
}

/// Adheres to the contract of the [Parse] trait, accepts one character and consumes trailing whitespace.
pub fn try_syntax(syntax: char, stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<()> {
    accept_if(|c| c == syntax, stream)?;
    skip_whitespace(stream)?;
    make(())
}

/// Similar to [try_syntax], but aborts parsing if the expected character is not found.
pub fn expect_syntax(
    syntax: char,
    stream: &mut Peekable<impl Iterator<Item = char>>,
) -> Parsed<()> {
    if try_syntax(syntax, stream).is_err() {
        let str = if let Some(c) = stream.peek() {
            c.to_string()
        } else {
            "EOL".to_string()
        };
        unrecoverable!("parse error: expecting `{syntax}' but found `{str}'")
    }
    make(())
}

/// Interface for working with types that implement the [Parse] trait; this allows parsing to use
/// type inference. Use this instead of calling [Parse::parse] directly.
pub fn try_nonterminal<T: Parse>(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<T> {
    let result = T::parse(stream)?;
    skip_whitespace(stream)?;
    make(result)
}

/// Interface for working with types that implement the [Parse] trait; this expects to parse
/// the given type or aborts parsing if not.
pub fn expect_nonterminal<T: Parse>(
    stream: &mut Peekable<impl Iterator<Item = char>>,
) -> Parsed<T> {
    force(try_nonterminal(stream))
}

/// Something that implements the Token trait is a token (i.e. a string of characters defined by a
/// maximum length, character classes, and possible escaping). The class for the first character of
/// the token can be different than that of the rest.
pub trait Token: Sized {
    const MAX_LEN: usize = 255;

    fn construct(s: String) -> Parsed<Self>;

    fn accept(c: char) -> bool;
    fn accept_1st(c: char) -> bool {
        Self::accept(c)
    }

    const ESCAPE: char = '\0';
    fn escaped(_: char) -> bool {
        false
    }
}

/// Implementation of the [Parse] trait for anything that implements [Token]
impl<T: Token> Parse for T {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        let accept_escaped = |pred: fn(char) -> bool, stream: &mut _| {
            if let Ok(c) = accept_if(pred, stream) {
                Ok(c)
            } else if accept_if(|c| c == T::ESCAPE, stream).is_ok() {
                if let Ok(c) = accept_if(T::escaped, stream) {
                    Ok(c)
                } else {
                    unrecoverable!("tokenizer: illegal escape sequence")
                }
            } else {
                reject()
            }
        };

        let mut str = accept_escaped(T::accept_1st, stream)?.to_string();
        while let Ok(c) = accept_escaped(T::accept, stream) {
            if str.len() >= T::MAX_LEN {
                unrecoverable!("tokenizer: exceeded safety margin")
            }
            str.push(c)
        }

        make(T::construct(str)?)
    }
}

/// Example parser for something that has two alternatives (don't use)
impl<T1: Token, T2: Parse> Parse for Result<T1, T2> {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        let &c = stream.peek().ok_or(Status::Reject)?;
        if T1::accept(c) {
            T1::parse(stream).map(Ok)
        } else {
            T2::parse(stream).map(Err)
        }
    }
}

/// Parsing method for lists of items separated by a given character; this adheres to the contract of the [Parse] trait.
fn parse_list<T: Parse>(
    sep_by: char,
    max: usize,
    stream: &mut Peekable<impl Iterator<Item = char>>,
) -> Parsed<Vec<T>> {
    let mut elems = Vec::new();
    elems.push(try_nonterminal(stream)?);
    while maybe(try_syntax(sep_by, stream))?.is_some() {
        if elems.len() >= max {
            unrecoverable!("parse_list: parsing multiple items: safety margin exceeded")
        }
        elems.push(expect_nonterminal(stream)?);
    }

    make(elems)
}

/// Types that implement the Many trait can be parsed multiple tokens into a `Vec<T>`; they are
/// seperated by `SEP`. There should also be a limit on the number of items.
pub trait Many {
    const SEP: char = ',';
    const LIMIT: usize = 127;
}

/// Generic implementation for parsing multiple items of a type `T` that implements the [Parse] and
/// [Many] traits.
impl<T: Parse + Many> Parse for Vec<T> {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<Self> {
        parse_list(T::SEP, T::LIMIT, stream)
    }
}

fn expect_end_of_parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<()> {
    if stream.peek().is_some() {
        unrecoverable!("parse error: trailing garbage")
    }
    make(())
}

pub fn expect_complete<T: Parse>(stream: &mut Peekable<impl Iterator<Item = char>>) -> Parsed<T> {
    let result = expect_nonterminal(stream)?;
    expect_end_of_parse(stream)?;
    make(result)
}

/// Convenience function (especially useful for writing test cases, to avoid having to write out the
/// AST constructors by hand.
pub fn parse_string<T: Parse>(text: &str) -> Parsed<T> {
    expect_complete(&mut text.chars().peekable())
}

#[cfg(test)]
pub fn parse_eval<T: Parse>(text: &str) -> T {
    parse_string(text).unwrap()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn comment_test() {
        assert_eq!(parse_eval::<Whitespace>(" # hello"), Whitespace);
    }
    #[test]
    #[should_panic]
    fn comment_test_fail() {
        assert_eq!(parse_eval::<Whitespace>(" # hello\nsomething"), Whitespace);
    }
}
