//! Lexer, parser and AST for `.weft` contracts. The formatter is added in task P1.5.
//!
//! ```
//! let file = weft_syntax::parse("orders.weft", "package orders\ntype Money { amount: int }").unwrap();
//! assert_eq!(file.package.name.name, "orders");
//! ```

pub mod ast;
mod format;
mod lexer;
mod parser;
pub mod token;

use chumsky::{Parser, Stream};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

pub use ast::File;
pub use format::{format, format_file};
pub use token::Span;

/// A syntax error with the source it points into, ready for miette to print.
#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(weft::syntax))]
pub struct SyntaxError {
    pub message: String,
    #[source_code]
    pub src: NamedSource<String>,
    #[label("{label}")]
    pub span: SourceSpan,
    pub label: String,
    #[help]
    pub help: Option<String>,
}

fn describe(token: Option<&token::Token>) -> String {
    token.map_or_else(|| "the end of the file".to_string(), ToString::to_string)
}

fn join_or(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} or {last}", init.join(", ")),
    }
}

/// Parse one `.weft` file. `name` is only used in error messages.
pub fn parse(name: &str, source: &str) -> Result<File, Vec<SyntaxError>> {
    let error = |message: String, label: String, span: Span, help: Option<String>| SyntaxError {
        message,
        src: NamedSource::new(name, source.to_string()),
        span: (span.start, span.end.saturating_sub(span.start)).into(),
        label,
        help,
    };

    let lexed = lexer::lex(source).map_err(|errors| {
        errors
            .into_iter()
            .map(|e| error(e.message, e.label, e.span, None))
            .collect::<Vec<_>>()
    })?;

    let end = source.len()..source.len();
    let stream = Stream::from_iter(end, lexed.tokens.into_iter());
    match parser::file().parse(stream) {
        Ok((package, imports, decls)) => Ok(File {
            package,
            imports,
            decls,
            comments: lexed.comments,
        }),
        Err(errors) => Err(errors
            .into_iter()
            .map(|e| {
                let found = describe(e.found());
                let mut many = false;
                let wanted = match e.label() {
                    Some(label) => label.to_string(),
                    None => {
                        let mut list: Vec<String> =
                            e.expected().map(|t| describe(t.as_ref())).collect();
                        list.sort();
                        list.dedup();
                        many = list.len() > 4;
                        join_or(&list)
                    }
                };
                let (message, label) = if wanted.is_empty() {
                    (format!("unexpected {found}"), "not valid here".to_string())
                } else {
                    (
                        format!("expected {wanted}, found {found}"),
                        if many {
                            "not valid here".to_string()
                        } else {
                            format!("expected {wanted}")
                        },
                    )
                };
                error(message, label, e.span(), None)
            })
            .collect()),
    }
}
