use miette::{Diagnostic, NamedSource, Report, SourceSpan};
use thiserror::Error;
use weft_syntax::Span;

use crate::ParsedFile;

/// A contract error: what is wrong, where, and often how to fix it.
#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(weft::check))]
pub struct CheckError {
    pub message: String,
    #[source_code]
    pub src: NamedSource<String>,
    #[label("{label}")]
    pub span: SourceSpan,
    pub label: String,
    #[help]
    pub help: Option<String>,
}

pub(crate) fn report(
    file: &ParsedFile,
    span: &Span,
    message: impl Into<String>,
    label: impl Into<String>,
    help: Option<String>,
) -> Report {
    Report::new(CheckError {
        message: message.into(),
        src: NamedSource::new(file.name.clone(), file.text.clone()),
        span: (span.start, span.end.saturating_sub(span.start)).into(),
        label: label.into(),
        help,
    })
}

/// The closest candidate to `name`, if one is close enough to be a likely typo.
pub(crate) fn suggest<'a>(
    name: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Option<&'a str> {
    let lower = name.to_lowercase();
    let mut best: Option<(&str, usize)> = None;
    for c in candidates {
        let d = distance(&lower, &c.to_lowercase());
        if d <= 2.max(name.len() / 4) && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((c, d));
        }
    }
    best.map(|(c, _)| c)
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// `order.placed` becomes `OrderPlaced`, `listOrders` becomes `ListOrders`.
pub(crate) fn pascal(text: &str) -> String {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut cs = s.chars();
            let first = cs.next().unwrap().to_ascii_uppercase();
            format!("{first}{}", cs.as_str())
        })
        .collect()
}
