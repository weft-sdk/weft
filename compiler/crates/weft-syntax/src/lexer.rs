//! Turns source text into tokens. Hand-written so that string and path errors point at exact bytes.

use crate::token::{Comment, Span, Token};

/// A lexical error: a message, the span it points at, and a label for that span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub label: String,
    pub span: Span,
}

pub struct Lexed {
    pub tokens: Vec<(Token, Span)>,
    pub comments: Vec<Comment>,
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '~' | '%')
}

pub fn lex(source: &str) -> Result<Lexed, Vec<LexError>> {
    let mut tokens = Vec::new();
    let mut comments = Vec::new();
    let mut errors = Vec::new();
    let mut chars = source.char_indices().peekable();

    while let Some((start, c)) = chars.next() {
        let next = chars.peek().map(|&(_, n)| n);
        let simple = |t: Token| Some((t, start..start + c.len_utf8()));
        let token = match c {
            c if c.is_whitespace() => None,
            '{' => simple(Token::LBrace),
            '}' => simple(Token::RBrace),
            '(' => simple(Token::LParen),
            ')' => simple(Token::RParen),
            '[' => simple(Token::LBracket),
            ']' => simple(Token::RBracket),
            '<' => simple(Token::Lt),
            '>' => simple(Token::Gt),
            ':' => simple(Token::Colon),
            ',' => simple(Token::Comma),
            '?' => simple(Token::Question),
            '|' => simple(Token::Pipe),
            '@' => simple(Token::At),
            '.' => simple(Token::Dot),
            '/' if next == Some('/') => {
                chars.next();
                let is_doc = chars.peek().map(|&(_, n)| n) == Some('/');
                if is_doc {
                    chars.next();
                }
                let body_start = chars.peek().map_or(source.len(), |&(i, _)| i);
                let mut end = body_start;
                while let Some(&(i, n)) = chars.peek() {
                    if n == '\n' {
                        break;
                    }
                    end = i + n.len_utf8();
                    chars.next();
                }
                let text = source[body_start..end].trim_end();
                if is_doc {
                    let text = text.strip_prefix(' ').unwrap_or(text);
                    Some((Token::Doc(text.to_string()), start..end))
                } else {
                    comments.push(Comment {
                        text: text.to_string(),
                        span: start..end,
                    });
                    None
                }
            }
            '/' => {
                let mut end = start + 1;
                while let Some(&(i, n)) = chars.peek() {
                    if !is_path_char(n) {
                        break;
                    }
                    end = i + n.len_utf8();
                    chars.next();
                }
                Some((Token::Path(source[start..end].to_string()), start..end))
            }
            '-' if next == Some('>') => {
                chars.next();
                Some((Token::Arrow, start..start + 2))
            }
            '-' | '0'..='9' => {
                let mut end = start + 1;
                let mut seen_dot = false;
                while let Some(&(i, n)) = chars.peek() {
                    let more = n.is_ascii_digit()
                        || (n == '.'
                            && !seen_dot
                            && source[i + 1..].starts_with(|d: char| d.is_ascii_digit()));
                    if !more {
                        break;
                    }
                    seen_dot |= n == '.';
                    end = i + n.len_utf8();
                    chars.next();
                }
                if end == start + 1 && c == '-' {
                    errors.push(LexError {
                        message: "unexpected `-`".into(),
                        label: "a `-` must start a negative number or an `->`".into(),
                        span: start..end,
                    });
                    None
                } else {
                    Some((Token::Num(source[start..end].to_string()), start..end))
                }
            }
            '"' => {
                let mut value = String::new();
                let mut closed = false;
                let mut end = start + 1;
                while let Some((i, n)) = chars.next() {
                    end = i + n.len_utf8();
                    match n {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\n' => {
                            end = i;
                            break;
                        }
                        '\\' => match chars.next() {
                            Some((j, e)) => {
                                end = j + e.len_utf8();
                                match e {
                                    '"' => value.push('"'),
                                    '\\' => value.push('\\'),
                                    'n' => value.push('\n'),
                                    't' => value.push('\t'),
                                    other => errors.push(LexError {
                                        message: format!("unknown escape `\\{other}`"),
                                        label: "write a backslash as `\\\\`, so a regex digit is `\\\\d`".into(),
                                        span: i..end,
                                    }),
                                }
                            }
                            None => break,
                        },
                        other => value.push(other),
                    }
                }
                if closed {
                    Some((Token::Str(value), start..end))
                } else {
                    errors.push(LexError {
                        message: "unterminated string".into(),
                        label: "this string has no closing `\"`".into(),
                        span: start..end,
                    });
                    None
                }
            }
            c if is_ident_start(c) => {
                let mut end = start + c.len_utf8();
                while let Some(&(i, n)) = chars.peek() {
                    if !is_ident_continue(n) {
                        break;
                    }
                    end = i + n.len_utf8();
                    chars.next();
                }
                Some((Token::Ident(source[start..end].to_string()), start..end))
            }
            other => {
                errors.push(LexError {
                    message: format!("unexpected character `{other}`"),
                    label: "not part of the contract language".into(),
                    span: start..start + other.len_utf8(),
                });
                None
            }
        };
        tokens.extend(token);
    }

    if errors.is_empty() {
        Ok(Lexed { tokens, comments })
    } else {
        Err(errors)
    }
}
