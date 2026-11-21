//! Tokens produced by the lexer.

use std::ops::Range;

/// A byte range in the source text.
pub type Span = Range<usize>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Token {
    /// A name. Keywords are ordinary identifiers: the parser decides by position.
    Ident(String),
    /// A string literal, with escapes resolved.
    Str(String),
    /// A number, kept as written (`-5`, `0.5`).
    Num(String),
    /// A bare path starting with `/`, as in `http /v1 {`.
    Path(String),
    /// A `///` documentation comment, without the slashes and one leading space.
    Doc(String),
    Arrow,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Lt,
    Gt,
    Colon,
    Comma,
    Question,
    Pipe,
    At,
    Dot,
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Ident(s) => write!(f, "`{s}`"),
            Token::Str(_) => f.write_str("a string"),
            Token::Num(n) => write!(f, "`{n}`"),
            Token::Path(p) => write!(f, "`{p}`"),
            Token::Doc(_) => f.write_str("a doc comment"),
            Token::Arrow => f.write_str("`->`"),
            Token::LBrace => f.write_str("`{`"),
            Token::RBrace => f.write_str("`}`"),
            Token::LParen => f.write_str("`(`"),
            Token::RParen => f.write_str("`)`"),
            Token::LBracket => f.write_str("`[`"),
            Token::RBracket => f.write_str("`]`"),
            Token::Lt => f.write_str("`<`"),
            Token::Gt => f.write_str("`>`"),
            Token::Colon => f.write_str("`:`"),
            Token::Comma => f.write_str("`,`"),
            Token::Question => f.write_str("`?`"),
            Token::Pipe => f.write_str("`|`"),
            Token::At => f.write_str("`@`"),
            Token::Dot => f.write_str("`.`"),
        }
    }
}

/// A `//` comment. Kept for the formatter; the parser never sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    /// The text after `//`, trimmed at the end.
    pub text: String,
    pub span: Span,
}
