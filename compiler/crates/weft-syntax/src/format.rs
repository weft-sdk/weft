//! The formatter: one canonical style for `.weft` files, keeping every comment.
//!
//! It prints the syntax tree and puts the `//` comments back by position: a comment on its own line stays above
//! the next item, a comment after an item on the same line stays after it. Blank lines are kept (at most one in a
//! row) and trailing comments in one block are aligned.

use crate::ast::*;
use crate::token::Comment;
use crate::{parse, SyntaxError};

const INDENT: &str = "  ";
const MAX_ONE_LINE_ENUM: usize = 80;

/// Format the text of one `.weft` file. `name` is only used in error messages.
pub fn format(name: &str, source: &str) -> Result<String, Vec<SyntaxError>> {
    let file = parse(name, source)?;
    Ok(format_file(&file, source))
}

/// Format a parsed file. `source` is the text it was parsed from, which is needed to see where blank lines and
/// same-line comments were.
pub fn format_file(file: &File, source: &str) -> String {
    let mut f = Fmt {
        src: source,
        comments: &file.comments,
        next: 0,
        lines: Vec::new(),
        depth: 0,
        group: 0,
        next_group: 1,
        prev_end: None,
    };
    f.file(file);
    f.finish()
}

struct Line {
    blank: bool,
    depth: usize,
    text: String,
    trailing: Option<String>,
    /// Lines in one group have their trailing comments aligned.
    group: usize,
}

struct Fmt<'a> {
    src: &'a str,
    comments: &'a [Comment],
    next: usize,
    lines: Vec<Line>,
    depth: usize,
    group: usize,
    next_group: usize,
    /// Where the previous item or comment ended, to see whether a blank line separated it from the next one.
    prev_end: Option<usize>,
}

impl Fmt<'_> {
    fn finish(mut self) -> String {
        self.lead(usize::MAX, false);
        // Align trailing comments within each group.
        let mut width: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for l in &self.lines {
            if l.trailing.is_some() {
                let w = l.depth * INDENT.len() + l.text.chars().count();
                let e = width.entry(l.group).or_insert(0);
                *e = (*e).max(w);
            }
        }
        let mut out = String::new();
        for l in &self.lines {
            if l.blank {
                out.push('\n');
                continue;
            }
            let mut s = INDENT.repeat(l.depth);
            s.push_str(&l.text);
            if let Some(t) = &l.trailing {
                let target = width[&l.group] + 2;
                let have = l.depth * INDENT.len() + l.text.chars().count();
                s.push_str(&" ".repeat(target - have));
                s.push_str(t);
            }
            out.push_str(&s);
            out.push('\n');
        }
        out
    }

    // ---- lines, blanks and comments ----

    fn push(&mut self, text: String) {
        self.lines.push(Line {
            blank: false,
            depth: self.depth,
            text,
            trailing: None,
            group: self.group,
        });
    }

    fn blank(&mut self) {
        if self.lines.last().is_some_and(|l| !l.blank) {
            self.lines.push(Line {
                blank: true,
                depth: 0,
                text: String::new(),
                trailing: None,
                group: 0,
            });
        }
    }

    fn gap_has_blank(&self, to: usize) -> bool {
        match self.prev_end {
            Some(e) if e <= to && to <= self.src.len() => {
                self.src[e..to].matches('\n').count() >= 2
            }
            _ => false,
        }
    }

    /// Emit every comment that sits before `start`, then a blank line if one belongs before the item.
    fn lead(&mut self, start: usize, force_blank: bool) {
        let mut first = true;
        while self.next < self.comments.len() && self.comments[self.next].span.end <= start {
            let c = &self.comments[self.next];
            if (first && force_blank) || self.gap_has_blank(c.span.start) {
                self.blank();
            }
            self.push(format!("//{}", c.text));
            self.prev_end = Some(c.span.end);
            self.next += 1;
            first = false;
        }
        if start != usize::MAX && ((first && force_blank) || self.gap_has_blank(start)) {
            self.blank();
        }
    }

    /// A comment after the item on the same line, if there is one.
    fn trailing(&mut self, end: usize) -> Option<String> {
        let c = self.comments.get(self.next)?;
        if c.span.start >= end
            && self.src[end..c.span.start]
                .chars()
                .all(|ch| matches!(ch, ' ' | '\t' | '\r' | ','))
        {
            self.next += 1;
            self.prev_end = Some(c.span.end);
            return Some(format!("//{}", c.text));
        }
        None
    }

    fn has_comment_within(&self, start: usize, end: usize) -> bool {
        self.comments[self.next..]
            .iter()
            .any(|c| c.span.start >= start && c.span.start < end)
    }

    /// Put an item on its own line(s): leading comments, the doc lines, the text, a trailing comment.
    fn item(&mut self, start: usize, end: usize, doc: &[String], text: String, force_blank: bool) {
        self.lead(start, force_blank);
        for d in doc {
            self.push(if d.is_empty() {
                "///".to_string()
            } else {
                format!("/// {d}")
            });
        }
        self.push(text);
        self.prev_end = Some(end);
        let t = self.trailing(end);
        if let Some(l) = self.lines.last_mut() {
            l.trailing = t;
        }
    }

    fn open_block(&mut self) -> (usize, Option<usize>) {
        self.depth += 1;
        let saved = (self.group, self.prev_end);
        self.group = self.next_group;
        self.next_group += 1;
        self.prev_end = None;
        (saved.0, saved.1)
    }

    /// Emit comments left before the closing brace, then leave the block.
    fn close_block(&mut self, saved: (usize, Option<usize>), close_at: usize) {
        self.lead(close_at, false);
        self.depth -= 1;
        self.group = saved.0;
        self.prev_end = saved.1;
    }

    fn fresh_group(&mut self) {
        self.group = self.next_group;
        self.next_group += 1;
    }

    // ---- the file ----

    fn file(&mut self, file: &File) {
        self.fresh_group();
        let p = &file.package;
        let mut text = format!("package {}", p.name.name);
        text.push_str(&annotations(&p.annotations));
        self.item(p.span.start, p.span.end, &[], text, false);
        for i in &file.imports {
            self.fresh_group();
            self.item(
                i.span.start,
                i.span.end,
                &[],
                format!("import {}", i.package.name),
                false,
            );
        }
        for d in &file.decls {
            self.fresh_group();
            self.decl(d);
        }
    }

    fn decl(&mut self, d: &Decl) {
        match &d.kind {
            DeclKind::Enum(en) => self.enum_decl(d, en),
            DeclKind::Type(t) => {
                let mut head = format!("type {}", t.name.name);
                if !t.params.is_empty() {
                    let ps: Vec<&str> = t.params.iter().map(|p| p.name.as_str()).collect();
                    head.push_str(&format!("<{}>", ps.join(", ")));
                }
                head.push_str(&annotations(&t.annotations));
                self.record_block(d, head, &t.fields);
            }
            DeclKind::Error(er) => {
                let head = format!("error {}{}", er.name.name, annotations(&er.annotations));
                self.record_block(d, head, &er.fields);
            }
            DeclKind::Event(ev) => self.event_decl(d, ev),
            DeclKind::Http(h) => self.http_decl(d, h),
            DeclKind::Rpc(r) => {
                self.block(d, format!("rpc {}", r.name.name), |f| {
                    for m in &r.methods {
                        let text = format!(
                            "{}{}{}{}",
                            m.name.name,
                            params(&m.params),
                            result(&m.result),
                            annotations(&m.annotations)
                        );
                        f.item(m.span.start, m.span.end, &m.doc, text, false);
                    }
                });
            }
            DeclKind::Graphql(g) => {
                let kw = g
                    .fields
                    .iter()
                    .map(|f| graphql_kind(f.kind).len())
                    .max()
                    .unwrap_or(0);
                self.block(d, "graphql".to_string(), |f| {
                    for x in &g.fields {
                        let text = format!(
                            "{:<kw$} {}{}{}{}",
                            graphql_kind(x.kind),
                            x.name.name,
                            params(&x.params),
                            result(&x.result),
                            annotations(&x.annotations)
                        );
                        f.item(x.span.start, x.span.end, &x.doc, text, false);
                    }
                });
            }
            DeclKind::Socket(sk) => {
                let width = sk
                    .events
                    .iter()
                    .map(|e| e.name.text().len() + 1)
                    .max()
                    .unwrap_or(0);
                self.block(d, format!("socket {}", sk.name.name), |f| {
                    for ev in &sk.events {
                        let side = match ev.side {
                            Side::Server => "server",
                            Side::Client => "client",
                        };
                        let name = format!("{}:", ev.name.text());
                        let ty = f.type_text(&ev.ty);
                        let text =
                            format!("{side} {name:<width$} {ty}{}", annotations(&ev.annotations));
                        f.item(ev.span.start, ev.span.end, &ev.doc, text, false);
                    }
                });
            }
        }
    }

    /// `head {` ... `}` around whatever `body` emits, as a top-level declaration.
    fn block(&mut self, d: &Decl, head: String, body: impl FnOnce(&mut Self)) {
        self.lead(d.span.start, true);
        for line in &d.doc {
            self.push(if line.is_empty() {
                "///".to_string()
            } else {
                format!("/// {line}")
            });
        }
        self.push(format!("{head} {{"));
        let opened = self.lines.len();
        let saved = self.open_block();
        body(self);
        self.close_block(saved, d.span.end.saturating_sub(1));
        if self.lines.len() == opened {
            // Nothing inside: print `head {}` on one line.
            self.lines.pop();
            self.push(format!("{head} {{}}"));
        } else {
            self.push("}".to_string());
        }
        self.prev_end = Some(d.span.end);
        let t = self.trailing(d.span.end);
        if let Some(l) = self.lines.last_mut() {
            l.trailing = t;
        }
    }

    fn record_block(&mut self, d: &Decl, head: String, fields: &[Field]) {
        self.block(d, head, |f| {
            for x in fields {
                f.field_item(x);
            }
        });
    }

    fn field_item(&mut self, x: &Field) {
        let text = self.field_text(x);
        self.item(x.span.start, x.span.end, &x.doc, text, false);
    }

    fn field_text(&mut self, x: &Field) -> String {
        format!(
            "{}{}: {}{}",
            x.name.name,
            if x.optional { "?" } else { "" },
            self.type_text(&x.ty),
            annotations(&x.annotations)
        )
    }

    fn enum_decl(&mut self, d: &Decl, e: &EnumDecl) {
        let head = format!("enum {}{}", e.name.name, annotations(&e.annotations));
        let one_line: Vec<String> = e
            .variants
            .iter()
            .map(|v| format!("{}{}", v.name.name, annotations(&v.annotations)))
            .collect();
        let single = format!("{head} {{ {} }}", one_line.join(" "));
        let simple = e.variants.iter().all(|v| v.doc.is_empty())
            && !self.has_comment_within(d.span.start, d.span.end)
            && single.len() <= MAX_ONE_LINE_ENUM
            && !e.variants.is_empty();
        if simple {
            self.item(d.span.start, d.span.end, &d.doc, single, true);
        } else {
            self.block(d, head, |f| {
                for v in &e.variants {
                    let text = format!("{}{}", v.name.name, annotations(&v.annotations));
                    f.item(v.span.start, v.span.end, &v.doc, text, false);
                }
            });
        }
    }

    fn event_decl(&mut self, d: &Decl, e: &EventDecl) {
        let mut head = format!("event {}: {}", e.name.text(), self.type_text(&e.ty));
        if e.options.is_empty() {
            head.push_str(&annotations(&e.annotations));
            self.item(d.span.start, d.span.end, &d.doc, head, true);
            return;
        }
        self.lead(d.span.start, true);
        for line in &d.doc {
            self.push(if line.is_empty() {
                "///".to_string()
            } else {
                format!("/// {line}")
            });
        }
        self.push(format!("{head} {{"));
        let saved = self.open_block();
        for o in &e.options {
            let (span, text) = match o {
                EventOption::Key { path, span } => (span, format!("key: {}", path.text())),
                EventOption::Headers { ty, span } => {
                    (span, format!("headers: {}", self.type_text(ty)))
                }
            };
            self.item(span.start, span.end, &[], text, false);
        }
        let last = match e.options.last() {
            Some(EventOption::Key { span, .. } | EventOption::Headers { span, .. }) => span.end,
            None => d.span.start,
        };
        let close = self.src[last..].find('}').map_or(d.span.end, |i| last + i);
        self.close_block(saved, close);
        self.push(format!("}}{}", annotations(&e.annotations)));
        self.prev_end = Some(d.span.end);
        let t = self.trailing(d.span.end);
        if let Some(l) = self.lines.last_mut() {
            l.trailing = t;
        }
    }

    fn http_decl(&mut self, d: &Decl, h: &HttpBlock) {
        let mw = h
            .routes
            .iter()
            .map(|r| r.method.name.len())
            .max()
            .unwrap_or(0);
        let pw = h
            .routes
            .iter()
            .map(|r| quote(&r.path.0).len())
            .max()
            .unwrap_or(0);
        let head = match &h.base {
            Some((b, _)) => format!("http {b}"),
            None => "http".to_string(),
        };
        self.block(d, head, |f| {
            for r in &h.routes {
                let lead = format!(
                    "{:<mw$} {:<pw$}  {}{}",
                    r.method.name,
                    quote(&r.path.0),
                    r.name.name,
                    params(&r.params)
                );
                let tail = format!(
                    "{}{}",
                    result(&r.result).trim_start(),
                    annotations(&r.annotations)
                );
                let one = format!("{lead} {tail}");
                if one.len() + f.depth * INDENT.len() <= 100 {
                    f.item(r.span.start, r.span.end, &r.doc, one, false);
                } else {
                    f.lead(r.span.start, false);
                    for line in &r.doc {
                        f.push(if line.is_empty() {
                            "///".to_string()
                        } else {
                            format!("/// {line}")
                        });
                    }
                    f.push(lead.trim_end().to_string());
                    f.depth += 1;
                    f.push(tail);
                    f.depth -= 1;
                    f.prev_end = Some(r.span.end);
                    let t = f.trailing(r.span.end);
                    if let Some(l) = f.lines.last_mut() {
                        l.trailing = t;
                    }
                }
            }
        });
    }

    // ---- types ----

    /// A type as text. An inline type is one line `{ a: T, b: U }` unless it holds comments or docs, in which
    /// case it is a block, with its lines joined by newlines.
    fn type_text(&mut self, t: &TypeExpr) -> String {
        match &t.kind {
            TypeKind::Named { path, args } => {
                let mut s = path.text();
                if !args.is_empty() {
                    let a: Vec<String> = args.iter().map(|a| self.type_text(a)).collect();
                    s.push_str(&format!("<{}>", a.join(", ")));
                }
                s
            }
            TypeKind::List(inner) => format!("{}[]", self.type_text(inner)),
            TypeKind::Optional(inner) => format!("{}?", self.type_text(inner)),
            TypeKind::Union(v) => {
                let parts: Vec<String> = v.iter().map(|x| self.type_text(x)).collect();
                parts.join(" | ")
            }
            TypeKind::Inline(fields) => {
                if fields.is_empty() {
                    return "{}".to_string();
                }
                let plain = fields.iter().all(|f| f.doc.is_empty())
                    && !self.has_comment_within(t.span.start, t.span.end);
                if plain {
                    let parts: Vec<String> = fields.iter().map(|f| self.field_text(f)).collect();
                    return format!("{{ {} }}", parts.join(", "));
                }
                // A block inside a line: the lines after the first carry their own indentation.
                let mut sub = Fmt {
                    src: self.src,
                    comments: self.comments,
                    next: self.next,
                    lines: Vec::new(),
                    depth: self.depth + 1,
                    group: self.next_group,
                    next_group: self.next_group + 1,
                    prev_end: None,
                };
                for f in fields {
                    sub.field_item(f);
                }
                sub.lead(t.span.end.saturating_sub(1), false);
                self.next = sub.next;
                self.next_group = sub.next_group;
                let inner = sub.finish_lines();
                let pad = INDENT.repeat(self.depth);
                format!("{{\n{inner}{pad}}}")
            }
        }
    }

    /// The lines of a sub-formatter as indented text, with trailing comments aligned.
    fn finish_lines(self) -> String {
        self.finish()
    }
}

fn graphql_kind(k: GraphqlKind) -> &'static str {
    match k {
        GraphqlKind::Query => "query",
        GraphqlKind::Mutation => "mutation",
        GraphqlKind::Subscription => "subscription",
    }
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn annotations(anns: &[Annotation]) -> String {
    let mut out = String::new();
    for a in anns {
        out.push_str(" @");
        out.push_str(&a.name.name);
        if !a.args.is_empty() {
            let args: Vec<String> = a
                .args
                .iter()
                .map(|x| match x {
                    AnnotationArg::Str(s) => quote(s),
                    AnnotationArg::Num(n) => n.clone(),
                    AnnotationArg::Ident(i) => i.clone(),
                })
                .collect();
            out.push_str(&format!("({})", args.join(", ")));
        }
    }
    out
}

/// `(a: T, b?: U)`, `(Req)`, `(stream Req)`, `()`.
fn params(ps: &[Param]) -> String {
    let parts: Vec<String> = ps
        .iter()
        .map(|p| {
            let ty = plain_type(&p.ty);
            match &p.name {
                Some(n) => format!("{}{}: {ty}", n.name, if p.optional { "?" } else { "" }),
                None if p.stream => format!("stream {ty}"),
                None => ty,
            }
        })
        .collect();
    format!("({})", parts.join(", "))
}

/// ` -> [stream] A | B? | C`
fn result(r: &ResultType) -> String {
    format!(
        " -> {}{}",
        if r.stream { "stream " } else { "" },
        plain_type(&r.ty)
    )
}

/// A type on one line, whatever it holds. Used where comments cannot occur (parameter lists and results).
fn plain_type(t: &TypeExpr) -> String {
    match &t.kind {
        TypeKind::Named { path, args } if args.is_empty() => path.text(),
        TypeKind::Named { path, args } => {
            let a: Vec<String> = args.iter().map(plain_type).collect();
            format!("{}<{}>", path.text(), a.join(", "))
        }
        TypeKind::List(i) => format!("{}[]", plain_type(i)),
        TypeKind::Optional(i) => format!("{}?", plain_type(i)),
        TypeKind::Union(v) => v.iter().map(plain_type).collect::<Vec<_>>().join(" | "),
        TypeKind::Inline(fields) if fields.is_empty() => "{}".to_string(),
        TypeKind::Inline(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|f| {
                    format!(
                        "{}{}: {}{}",
                        f.name.name,
                        if f.optional { "?" } else { "" },
                        plain_type(&f.ty),
                        annotations(&f.annotations)
                    )
                })
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
    }
}
