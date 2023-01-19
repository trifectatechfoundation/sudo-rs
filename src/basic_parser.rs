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
//! ```rust
//! impl Parse of LinkedList<u32> {
//!     fn parse(stream: ...) -> Option<Vec<u32>> {
//!         let x = is_some::<u32>(stream)?;
//!         let mut tail = if try_syntax('+', stream).is_some() {
//!             expect_nonterminal::<LinkedList<u32>>(stream);
//!             rest
//!         } else {
//!             LinkedList::new()
//!         }
//!         rest.push_front(x);
//!     }
//! }
//! ```

// TODO: whitespace discipline may be better moved to "try_***" methods instead of the parse trait

use std::iter::Peekable;

/// All implementations of the Parse trait must satisfy this contract:
///
/// If the `parse` method of this trait returns None, the iterator is not advanced; otherwise it is
/// advanced beyond the accepted part of the input. i.e. if some input is consumed the method
/// *MUST* be producing a `Some` value.
///
/// Implementations of this trait should consume trailing whitespace as well.
pub trait Parse {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self>
    where
        Self: Sized;
}

/// Convenience function (especially useful for writing test cases, to avoid having to write out the
/// AST constructors by hand.
#[cfg(test)]
pub fn parse_eval<T: Parse>(text: &str) -> T {
    expect_complete(&mut text.chars().peekable())
}

/// Primitive function (that also adheres to the Parse trait contract): accepts one character
/// that satisfies `predicate`. This is used in the majority of all the other `Parse`
/// implementations instead of interfacing with the iterator directly (this can facilitate an easy
/// switch to a different method of stream representation in the future). Unlike most `Parse`
/// implementations this *does not* consume trailing whitespace.
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
/// A structure representing whitespace
struct Whitespace;

/// Accept one or more whitespace characters; fails if no whitespace is found (to parse zero or
/// more whitespace characters, parse `Option<Whitespace>`
impl Parse for Whitespace {
    fn parse(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<Self> {
        let mut eat_space = || accept_if(char::is_whitespace, stream);
        eat_space()?;
        while eat_space().is_some() {}
        Some(Whitespace {})
    }
}

/// Adheres to the contract of the [Parse] trait, accepts one character and consumes following whitespace.
pub fn try_syntax(syntax: char, stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<()> {
    accept_if(|c| c == syntax, stream)?;
    Whitespace::parse(stream);
    Some(())
}

/// Similar to [try_syntax], but aborts parsing if the expected character is not found.
pub fn expect_syntax(syntax: char, stream: &mut Peekable<impl Iterator<Item = char>>) {
    if try_syntax(syntax, stream).is_none() {
        let str = if let Some(c) = stream.peek() {
            c.to_string()
        } else {
            "EOL".to_string()
        };
        panic!("parse error: expecting `{syntax}' but found `{str}'")
    }
}

/// Interface for working with types that implement the [Parse] trait; this allows parsing to use
/// type inference. Use this instead of calling [Parse::parse] directly.
pub fn try_nonterminal<T: Parse>(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<T> {
    T::parse(stream)
}

/// Interface for working with types that implement the [Parse] trait; this expects to parse
/// the given type or aborts parsing if not.
pub fn expect_nonterminal<T: Parse>(stream: &mut Peekable<impl Iterator<Item = char>>) -> T {
    let Some(result) = try_nonterminal(stream) else {
        panic!("parse error: expected `{}'", std::any::type_name::<T>())
    };
    result
}

/// Something that implements the Token trait is a token (i.e. a string of characters defined by a
/// maximum length, character classes, and possible escaping). The class for the first character of
/// the token can be different than that of the rest.
pub trait Token {
    const IDENT: fn(String) -> Self; // make this a regular function, or require Token to extend From?
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

/// Implementation of the [Parse] trait for anything that implements [Token]
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

/// Example parser for something that has two alternatives (don't use)
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

/// Parsing method for lists of items separated by a given character; this adheres to the contract of the [Parse] trait.
fn parse_list<T: Parse>(
    sep_by: char,
    max: usize,
    stream: &mut Peekable<impl Iterator<Item = char>>,
) -> Option<Vec<T>> {
    let mut elems = Vec::new();
    elems.push(try_nonterminal(stream)?);
    while try_syntax(sep_by, stream).is_some() {
        if elems.len() >= max {
            panic!("parse_list: parsing multiple items: safety margin exceeded")
        }
        elems.push(expect_nonterminal(stream));
    }
    Some(elems)
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
pub fn try_complete<T: Parse>(stream: &mut Peekable<impl Iterator<Item = char>>) -> Option<T> {
    let result = try_nonterminal(stream)?;
    expect_end_of_parse(stream);
    Some(result)
}

#[allow(dead_code)]
pub fn expect_complete<T: Parse>(stream: &mut Peekable<impl Iterator<Item = char>>) -> T {
    let result = expect_nonterminal(stream);
    expect_end_of_parse(stream);
    result
}
