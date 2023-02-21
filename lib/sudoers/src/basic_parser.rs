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
//!     fn parse(stream: &mut impl CharStream) -> Parsed<LinkedList<T>> {
//!         let x = try_nonterminal(stream)?;
//!         let mut tail = if is_syntax('+', stream)? {
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

/// Type holding a parsed object (or error information if parsing failed)
pub type Parsed<T> = Result<T, Status>;

pub type Position = (usize, usize);

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Status {
    Fatal(Position, String), // not recoverable; stream in inconsistent state
    Reject,                  // parsing failed by no input consumed
}

pub fn make<T>(value: T) -> Parsed<T> {
    Ok(value)
}

pub fn reject<T>() -> Parsed<T> {
    Err(Status::Reject)
}

macro_rules! unrecoverable {
    ($stream:ident, $($str:expr),*) => {
        return Err(crate::basic_parser::Status::Fatal($stream.get_pos(),format![$($str),*]))
    };
    ($($str:expr),*) => {
        return Err(crate::basic_parser::Status::Fatal(None,format![$($str),*]))
    };
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

pub use crate::char_stream::CharStream;

/// All implementations of the Parse trait must satisfy this contract:
///
/// If the `parse` method of this trait returns None, the iterator is not advanced; otherwise it is
/// advanced beyond the accepted part of the input. i.e. if some input is consumed the method
/// *MUST* be producing a `Some` value.
pub trait Parse {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self>
    where
        Self: Sized;
}

/// Primitive function (that also adheres to the Parse trait contract): accepts one character
/// that satisfies `predicate`. This is used in the majority of all the other `Parse`
/// implementations instead of interfacing with the iterator directly (this can facilitate an easy
/// switch to a different method of stream representation in the future). Unlike most `Parse`
/// implementations this *does not* consume trailing whitespace.
/// NOTE: Guaranteed not to give an unrecoverable error.
pub fn accept_if(predicate: impl Fn(char) -> bool, stream: &mut impl CharStream) -> Parsed<char> {
    let c = stream.peek().ok_or(Status::Reject)?;
    if predicate(c) {
        stream.advance();
        make(c)
    } else {
        reject()
    }
}

/// Structures representing whitespace (trailing whitespace can contain comments)
#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
struct LeadingWhitespace;

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
struct TrailingWhitespace;

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
struct Comment;

/// Accept zero or more whitespace characters; fails if the whitespace is not "leading" to something
/// (which can be used to detect end-of-input).
impl Parse for LeadingWhitespace {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        let eat_space = |stream: &mut _| accept_if(|c| "\t ".contains(c), stream);
        while eat_space(stream).is_ok() {}

        if stream.peek().is_some() {
            make(LeadingWhitespace {})
        } else {
            unrecoverable!(stream, "superfluous whitespace")
        }
    }
}

/// Accept zero or more whitespace characters; since this accepts zero characters, it
/// always succeeds (unless some serious error occurs). This parser also accepts comments,
/// since those can form part of trailing white space.
impl Parse for TrailingWhitespace {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        loop {
            let _ = LeadingWhitespace::parse(stream); // don't propagate any errors

            // line continuations
            if accept_if(|c| c == '\\', stream).is_ok() {
                // do the equivalent of expect_syntax('\n', stream)?, without recursion
                if accept_if(|c| c == '\n', stream).is_err() {
                    unrecoverable!(stream, "stray escape sequence")
                }
            } else {
                break;
            }
        }

        make(TrailingWhitespace {})
    }
}

/// Parses a comment
impl Parse for Comment {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        accept_if(|c| c == '#', stream)?;
        while accept_if(|c| c != '\n', stream).is_ok() {}
        make(Comment {})
    }
}

fn skip_trailing_whitespace(stream: &mut impl CharStream) -> Parsed<()> {
    TrailingWhitespace::parse(stream)?;
    make(())
}

/// Adheres to the contract of the [Parse] trait, accepts one character and consumes trailing whitespace.
pub fn try_syntax(syntax: char, stream: &mut impl CharStream) -> Parsed<()> {
    accept_if(|c| c == syntax, stream)?;
    skip_trailing_whitespace(stream)?;
    make(())
}

/// Similar to [try_syntax], but aborts parsing if the expected character is not found.
pub fn expect_syntax(syntax: char, stream: &mut impl CharStream) -> Parsed<()> {
    if try_syntax(syntax, stream).is_err() {
        let str = if let Some(c) = stream.peek() {
            c.to_string()
        } else {
            "EOF".to_string()
        };
        unrecoverable!(
            stream,
            "parse error: expecting `{syntax}' but found `{str}'"
        )
    }
    make(())
}

/// Convenience function: usually try_syntax is called as a test criterion; if this returns true, the input was consumed.
pub fn is_syntax(syntax: char, stream: &mut impl CharStream) -> Parsed<bool> {
    let result = maybe(try_syntax(syntax, stream))?;
    make(result.is_some())
}

/// Interface for working with types that implement the [Parse] trait; this allows parsing to use
/// type inference. Use this instead of calling [Parse::parse] directly.
pub fn try_nonterminal<T: Parse>(stream: &mut impl CharStream) -> Parsed<T> {
    let result = T::parse(stream)?;
    skip_trailing_whitespace(stream)?;
    make(result)
}

/// Interface for working with types that implement the [Parse] trait; this expects to parse
/// the given type or aborts parsing if not.
pub fn expect_nonterminal<T: Parse>(stream: &mut impl CharStream) -> Parsed<T> {
    match try_nonterminal(stream) {
        Err(Status::Reject) => {
            unrecoverable!(
                stream,
                "parse error: expected `{}'",
                std::any::type_name::<T>()
            )
        }
        result => result,
    }
}

/// Something that implements the Token trait is a token (i.e. a string of characters defined by a
/// maximum length, character classes, and possible escaping). The class for the first character of
/// the token can be different than that of the rest.
pub trait Token: Sized {
    const MAX_LEN: usize = 255;

    fn construct(s: String) -> Result<Self, String>;

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
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        fn accept_escaped<T: Token>(
            pred: fn(char) -> bool,
            stream: &mut impl CharStream,
        ) -> Parsed<char> {
            if let Ok(c) = accept_if(pred, stream) {
                Ok(c)
            } else if accept_if(|c| c == T::ESCAPE, stream).is_ok() {
                if let Ok(c) = accept_if(T::escaped, stream) {
                    Ok(c)
                } else {
                    unrecoverable!(stream, "tokenizer: illegal escape sequence")
                }
            } else {
                reject()
            }
        }

        let mut str = accept_escaped::<T>(T::accept_1st, stream)?.to_string();
        while let Ok(c) = accept_escaped::<T>(T::accept, stream) {
            if str.len() >= T::MAX_LEN {
                unrecoverable!(stream, "tokenizer: exceeded safety margin")
            }
            str.push(c)
        }

        match T::construct(str) {
            Ok(result) => make(result),
            Err(msg) => unrecoverable!(stream, "{msg}"),
        }
    }
}

/// Parser for Option<T> (this can be used to make the code more readable)
impl<T: Parse> Parse for Option<T> {
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        maybe(T::parse(stream))
    }
}

/// Parsing method for lists of items separated by a given character; this adheres to the contract of the [Parse] trait.
fn parse_list<T: Parse>(sep_by: char, max: usize, stream: &mut impl CharStream) -> Parsed<Vec<T>> {
    let mut elems = Vec::new();
    elems.push(try_nonterminal(stream)?);
    while maybe(try_syntax(sep_by, stream))?.is_some() {
        if elems.len() >= max {
            unrecoverable!(
                stream,
                "parse_list: parsing multiple items: safety margin exceeded"
            )
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
    fn parse(stream: &mut impl CharStream) -> Parsed<Self> {
        parse_list(T::SEP, T::LIMIT, stream)
    }
}

/// Entry point utility function; parse a Vec<T> but with fatal error recovery per line
pub fn parse_lines<T: Parse, Stream: CharStream>(stream: &mut Stream) -> Vec<Parsed<T>> {
    let mut result = Vec::new();

    // this will terminate; if the inner accept_if is an error, either a character will be consumed
    // by the second accept_if (making progress), or the end of the stream will have been reacherd
    // (which will cause the next iteration to fall through)

    while LeadingWhitespace::parse(stream).is_ok() {
        let item = expect_nonterminal(stream);
        let parsed_item_ok = item.is_ok();
        result.push(item);

        let _ = maybe(Comment::parse(stream));
        if accept_if(|c| c == '\n', stream).is_err() {
            if parsed_item_ok {
                let msg = if stream.peek().is_none() {
                    "parse error: missing line terminator at end of file"
                } else {
                    "parse error: garbage at end of line"
                };
                let error = |stream: &mut Stream| unrecoverable!(stream, "{msg}");
                result.push(error(stream));
            }
            while accept_if(|c| c != '\n', stream).is_ok() {}
        }
    }

    result
}

#[cfg(test)]
fn expect_complete<T: Parse>(stream: &mut impl CharStream) -> Parsed<T> {
    let result = expect_nonterminal(stream)?;
    if let Some(c) = stream.peek() {
        unrecoverable!(stream, "parse error: garbage at end of line: {c}")
    }
    make(result)
}

/// Convenience function (especially useful for writing test cases, to avoid having to write out the
/// AST constructors by hand.
#[cfg(test)]
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

    impl Token for String {
        fn construct(val: String) -> Result<Self, String> {
            Ok(val)
        }

        fn accept(c: char) -> bool {
            c.is_ascii_alphanumeric()
        }
    }

    impl Many for String {}

    #[test]
    fn comment_test() {
        assert_eq!(parse_eval::<Comment>("# hello"), Comment);
    }
    #[test]
    #[should_panic]
    fn comment_test_fail() {
        assert_eq!(parse_eval::<Comment>("# hello\nsomething"), Comment);
    }

    #[test]
    fn lines_test() {
        let input = |text: &str| parse_lines(&mut text.chars().peekable());

        let s = |text: &str| Ok(text.to_string());
        assert_eq!(input("hello\nworld\n"), vec![s("hello"), s("world")]);
        assert_eq!(input("   hello\nworld\n"), vec![s("hello"), s("world")]);
        assert_eq!(input("hello  \nworld\n"), vec![s("hello"), s("world")]);
        assert_eq!(input("hello\n   world\n"), vec![s("hello"), s("world")]);
        assert_eq!(input("hello\nworld  \n"), vec![s("hello"), s("world")]);
        assert_eq!(input("hello\nworld")[0..2], vec![s("hello"), s("world")]);
        let Err(_) = input("hello\nworld")[2] else { panic!() };
        let Err(_) = input("hello\nworld:\n")[2] else { panic!() };
    }
    #[test]
    fn whitespace_test() {
        assert_eq!(
            parse_eval::<Vec<String>>("hello,something"),
            vec!["hello", "something"]
        );
        assert_eq!(
            parse_eval::<Vec<String>>("hello , something"),
            vec!["hello", "something"]
        );
        assert_eq!(
            parse_eval::<Vec<String>>("hello, something"),
            vec!["hello", "something"]
        );
        assert_eq!(
            parse_eval::<Vec<String>>("hello ,something"),
            vec!["hello", "something"]
        );
        assert_eq!(
            parse_eval::<Vec<String>>("hello\\\n,something"),
            vec!["hello", "something"]
        );
    }
}
