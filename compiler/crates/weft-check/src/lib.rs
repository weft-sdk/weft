//! Name resolution and type checking for `.weft` contracts. Produces one [`Ir`] per package.
//!
//! ```
//! let sources = [weft_check::Source::new("orders.weft", "package orders\ntype Money { amount: int }")];
//! let irs = weft_check::check(&sources).unwrap();
//! assert_eq!(irs[0].package, "orders");
//! ```

mod annotations;
mod error;
mod lower;
mod ops;
mod resolve;

use std::collections::{BTreeMap, HashMap};

use miette::Report;
use weft_ir::{Ir, BUILTIN_PACKAGE};
use weft_syntax::ast::{DeclKind, File};

pub use error::CheckError;

/// One `.weft` file: its name (for messages) and its text.
#[derive(Clone, Debug)]
pub struct Source {
    pub name: String,
    pub text: String,
}

impl Source {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        Source {
            name: name.into(),
            text: text.into(),
        }
    }
}

pub(crate) struct ParsedFile {
    pub name: String,
    pub text: String,
    pub ast: File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntityKind {
    Record,
    Enum,
    Error,
}

/// A declared name that other declarations may refer to.
#[derive(Clone, Debug)]
pub(crate) struct Entity {
    pub kind: EntityKind,
    /// Number of type parameters.
    pub arity: usize,
    /// The file that declares it, for messages.
    pub file: String,
}

/// The names a package declares.
#[derive(Default)]
pub(crate) struct PackageIndex {
    pub entities: HashMap<String, Entity>,
}

/// Check a set of `.weft` files that make up one or more packages.
///
/// Every package any file imports must be among the files. On success there is one IR per package, sorted by
/// package name. On failure there is every error found, which may be many.
pub fn check(sources: &[Source]) -> Result<Vec<Ir>, Vec<Report>> {
    let mut errors: Vec<Report> = Vec::new();

    let mut files = Vec::new();
    for s in sources {
        match weft_syntax::parse(&s.name, &s.text) {
            Ok(ast) => files.push(ParsedFile {
                name: s.name.clone(),
                text: s.text.clone(),
                ast,
            }),
            Err(syntax) => errors.extend(syntax.into_iter().map(Report::new)),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, f) in files.iter().enumerate() {
        groups
            .entry(f.ast.package.name.name.clone())
            .or_default()
            .push(i);
    }

    let mut indexes: HashMap<String, PackageIndex> = HashMap::new();
    for (package, members) in &groups {
        let mut index = PackageIndex::default();
        for &i in members {
            let file = &files[i];
            if package == BUILTIN_PACKAGE {
                errors.push(error::report(
                    file,
                    &file.ast.package.name.span,
                    format!("`{BUILTIN_PACKAGE}` is reserved for built-in errors"),
                    "choose another package name",
                    None,
                ));
            }
            for d in &file.ast.decls {
                let (name, kind, arity) = match &d.kind {
                    DeclKind::Enum(e) => (&e.name, EntityKind::Enum, 0),
                    DeclKind::Type(t) => (&t.name, EntityKind::Record, t.params.len()),
                    DeclKind::Error(e) => (&e.name, EntityKind::Error, 0),
                    _ => continue,
                };
                if let Some(first) = index.entities.get(&name.name) {
                    errors.push(error::report(
                        file,
                        &name.span,
                        format!("`{}` is declared twice in package `{package}`", name.name),
                        "already declared",
                        Some(format!("it is first declared in {}", first.file)),
                    ));
                } else {
                    index.entities.insert(
                        name.name.clone(),
                        Entity {
                            kind,
                            arity,
                            file: file.name.clone(),
                        },
                    );
                }
            }
        }
        indexes.insert(package.clone(), index);
    }

    let order = import_order(&files, &groups, &mut errors);

    let mut done: HashMap<String, Ir> = HashMap::new();
    for package in order {
        let members: Vec<&ParsedFile> = groups[&package].iter().map(|&i| &files[i]).collect();
        let (ir, package_errors) = lower::lower_package(&package, &members, &indexes, &done);
        errors.extend(package_errors);
        done.insert(package, ir);
    }

    if errors.is_empty() {
        let mut irs: Vec<Ir> = done.into_values().collect();
        irs.sort_by(|a, b| a.package.cmp(&b.package));
        Ok(irs)
    } else {
        Err(errors)
    }
}

/// Packages in an order where every package comes after the packages it imports. Reports unknown imports and
/// import cycles.
fn import_order(
    files: &[ParsedFile],
    groups: &BTreeMap<String, Vec<usize>>,
    errors: &mut Vec<Report>,
) -> Vec<String> {
    let mut edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (package, members) in groups {
        let deps = edges.entry(package.as_str()).or_default();
        for &i in members {
            let file = &files[i];
            for imp in &file.ast.imports {
                let name = imp.package.name.as_str();
                if name == package {
                    errors.push(error::report(
                        file,
                        &imp.span,
                        format!("package `{package}` imports itself"),
                        "remove this import",
                        None,
                    ));
                } else if !groups.contains_key(name) {
                    let known: Vec<&str> = groups.keys().map(String::as_str).collect();
                    errors.push(error::report(
                        file,
                        &imp.package.span,
                        format!("unknown package `{name}`"),
                        "no file declares this package",
                        Some(match error::suggest(name, known.iter().copied()) {
                            Some(s) => format!("did you mean `{s}`?"),
                            None => {
                                "add the package's contracts to `contracts:` in weft.yaml".into()
                            }
                        }),
                    ));
                } else if !deps.contains(&name) {
                    deps.push(name);
                }
            }
        }
    }

    #[derive(PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    fn visit<'a>(
        node: &'a str,
        edges: &BTreeMap<&'a str, Vec<&'a str>>,
        marks: &mut HashMap<&'a str, Mark>,
        out: &mut Vec<String>,
        cycle: &mut Option<String>,
    ) {
        match marks.get(node) {
            Some(Mark::Done) => return,
            Some(Mark::Visiting) => {
                cycle.get_or_insert_with(|| node.to_string());
                return;
            }
            None => {}
        }
        marks.insert(node, Mark::Visiting);
        for &dep in edges.get(node).into_iter().flatten() {
            visit(dep, edges, marks, out, cycle);
        }
        marks.insert(node, Mark::Done);
        out.push(node.to_string());
    }

    let mut marks = HashMap::new();
    let mut out = Vec::new();
    let mut cycle = None;
    for &node in edges.keys() {
        visit(node, &edges, &mut marks, &mut out, &mut cycle);
    }
    if let Some(package) = cycle {
        let file = &files[groups[&package][0]];
        errors.push(error::report(
            file,
            &file.ast.package.name.span,
            format!("package `{package}` is part of an import cycle"),
            "this package imports, directly or not, a package that imports it",
            Some("move the shared types into a third package".into()),
        ));
    }
    out
}
