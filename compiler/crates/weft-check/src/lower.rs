//! Lowering of one package: declarations become IR.

use std::collections::{HashMap, HashSet};

use miette::Report;
use weft_ir::{
    ErrorDef, FieldDef, Ir, TypeBody, TypeDef, VariantDef, BUILTIN_ERRORS,
    DEFAULT_CUSTOM_ERROR_STATUS,
};
use weft_syntax::ast::{self as a, DeclKind};
use weft_syntax::Span;

use crate::annotations::Target;
use crate::error::{pascal, report};
use crate::resolve::prim_from;
use crate::{PackageIndex, ParsedFile};

pub(crate) struct Lower<'a> {
    pub package: &'a str,
    pub indexes: &'a HashMap<String, PackageIndex>,
    /// Packages already lowered (the ones this package may import).
    pub done: &'a HashMap<String, Ir>,
    pub ir: Ir,
    pub errors: Vec<Report>,
    pub file: &'a ParsedFile,
    /// Packages the current file imports.
    pub imports: Vec<String>,
    /// Type parameters in scope.
    pub generics: Vec<String>,
    /// Every type name in use: declared ones and generated ones.
    pub taken: HashSet<String>,
    pub event_names: HashSet<String>,
    pub route_names: HashSet<String>,
    pub route_shapes: HashSet<String>,
    pub services: HashSet<String>,
    pub sockets: HashSet<String>,
}

pub(crate) fn lower_package(
    package: &str,
    members: &[&ParsedFile],
    indexes: &HashMap<String, PackageIndex>,
    done: &HashMap<String, Ir>,
) -> (Ir, Vec<Report>) {
    let mut l = Lower {
        package,
        indexes,
        done,
        ir: Ir::new(package),
        errors: Vec::new(),
        file: members[0],
        imports: Vec::new(),
        generics: Vec::new(),
        taken: indexes[package].entities.keys().cloned().collect(),
        event_names: HashSet::new(),
        route_names: HashSet::new(),
        route_shapes: HashSet::new(),
        services: HashSet::new(),
        sockets: HashSet::new(),
    };

    let mut version_from: Option<&str> = None;
    for &f in members {
        l.set_file(f);
        let anns = l.annotations(&f.ast.package.annotations, Target::Package, None);
        if let Some(v) = anns.version {
            match &l.ir.version {
                Some(existing) if *existing != v => l.error(
                    &f.ast.package.span,
                    format!("package `{package}` has two versions: {existing} and {v}"),
                    "conflicting `@version`",
                    Some(format!(
                        "{} sets {existing}; keep `@version` in one file",
                        version_from.unwrap_or("another file")
                    )),
                ),
                Some(_) => {}
                None => {
                    l.ir.version = Some(v);
                    version_from = Some(&f.name);
                }
            }
        }
        for imp in &f.ast.imports {
            if !l.ir.imports.contains(&imp.package.name) && done.contains_key(&imp.package.name) {
                l.ir.imports.push(imp.package.name.clone());
            }
        }
    }

    for &f in members {
        l.set_file(f);
        for d in &f.ast.decls {
            l.lower_type_decl(d);
        }
    }
    for &f in members {
        l.set_file(f);
        for d in &f.ast.decls {
            l.lower_operation_decl(d);
        }
    }

    (l.ir, l.errors)
}

impl<'a> Lower<'a> {
    pub fn set_file(&mut self, file: &'a ParsedFile) {
        self.file = file;
        self.imports = file
            .ast
            .imports
            .iter()
            .map(|i| i.package.name.clone())
            .collect();
    }

    pub fn error(
        &mut self,
        span: &Span,
        message: impl Into<String>,
        label: impl Into<String>,
        help: Option<String>,
    ) {
        self.errors
            .push(report(self.file, span, message, label, help));
    }

    fn lower_type_decl(&mut self, d: &a::Decl) {
        match &d.kind {
            DeclKind::Enum(e) => self.lower_enum(d, e),
            DeclKind::Type(t) => self.lower_record(d, t),
            DeclKind::Error(e) => self.lower_error(d, e),
            _ => {}
        }
    }

    fn check_declared_name(&mut self, name: &a::Ident) {
        let reserved = prim_from(&name.name).is_some()
            || name.name == "map"
            || name.name == "stream"
            || BUILTIN_ERRORS.iter().any(|(n, _)| *n == name.name);
        if reserved {
            self.error(
                &name.span,
                format!("`{}` is a built-in name", name.name),
                "choose another name",
                None,
            );
        }
    }

    fn lower_enum(&mut self, d: &a::Decl, e: &a::EnumDecl) {
        self.check_declared_name(&e.name);
        let anns = self.annotations(&e.annotations, Target::Enum, None);
        if e.variants.is_empty() {
            self.error(
                &e.name.span,
                format!("enum `{}` has no values", e.name.name),
                "an enum needs at least one value",
                None,
            );
        }
        let mut seen = HashSet::new();
        let mut variants = Vec::new();
        for v in &e.variants {
            if !seen.insert(v.name.name.clone()) {
                self.error(
                    &v.name.span,
                    format!("`{}` appears twice in `{}`", v.name.name, e.name.name),
                    "duplicate value",
                    None,
                );
                continue;
            }
            let va = self.annotations(&v.annotations, Target::Variant, None);
            variants.push(VariantDef {
                name: v.name.name.clone(),
                doc: v.doc.clone(),
                deprecated: va.deprecated,
            });
        }
        self.ir.types.push(TypeDef {
            name: e.name.name.clone(),
            doc: d.doc.clone(),
            params: vec![],
            inline: false,
            deprecated: anns.deprecated,
            body: TypeBody::Enum { variants },
        });
    }

    fn lower_record(&mut self, d: &a::Decl, t: &a::TypeDecl) {
        self.check_declared_name(&t.name);
        let anns = self.annotations(&t.annotations, Target::Type, None);
        let mut params: Vec<String> = Vec::new();
        for p in &t.params {
            if params.contains(&p.name) {
                self.error(
                    &p.span,
                    format!("type parameter `{}` is declared twice", p.name),
                    "duplicate",
                    None,
                );
            } else if prim_from(&p.name).is_some() || p.name == "map" {
                self.error(
                    &p.span,
                    format!("`{}` is a built-in name", p.name),
                    "choose another name for the type parameter",
                    None,
                );
            } else {
                params.push(p.name.clone());
            }
        }
        self.generics = params.clone();
        let fields = self.lower_fields(&t.fields, &t.name.name, "type");
        self.generics.clear();
        self.ir.types.push(TypeDef {
            name: t.name.name.clone(),
            doc: d.doc.clone(),
            params,
            inline: false,
            deprecated: anns.deprecated,
            body: TypeBody::Record { fields },
        });
    }

    fn lower_error(&mut self, d: &a::Decl, e: &a::ErrorDecl) {
        self.check_declared_name(&e.name);
        let anns = self.annotations(&e.annotations, Target::Error, None);
        let fields = self.lower_fields(&e.fields, &e.name.name, "error");
        self.ir.errors.push(ErrorDef {
            name: e.name.name.clone(),
            doc: d.doc.clone(),
            status: anns.status.unwrap_or(DEFAULT_CUSTOM_ERROR_STATUS),
            fields,
        });
    }

    /// Lower a list of fields. `owner` names the enclosing type and starts the names of inline types.
    pub fn lower_fields(&mut self, fields: &[a::Field], owner: &str, what: &str) -> Vec<FieldDef> {
        let mut names = HashSet::new();
        let mut wires = HashSet::new();
        let mut out = Vec::new();
        for f in fields {
            let dup_name = !names.insert(f.name.name.clone());
            if dup_name {
                self.error(
                    &f.name.span,
                    format!(
                        "field `{}` is declared twice in {what} `{owner}`",
                        f.name.name
                    ),
                    "duplicate field",
                    None,
                );
            }
            let ctx = format!("{owner}{}", pascal(&f.name.name));
            let ty = self.resolve_type(&f.ty, &ctx);
            let anns = self.annotations(&f.annotations, Target::Field, ty.as_ref());
            let Some(ty) = ty else { continue };
            if dup_name {
                continue;
            }
            let wire_name = anns.wire.unwrap_or_else(|| f.name.name.clone());
            if !wires.insert(wire_name.clone()) {
                self.error(
                    &f.name.span,
                    format!("two fields of `{owner}` are called `{wire_name}` on the wire"),
                    "duplicate wire name",
                    Some("check the `@wire` annotations".into()),
                );
                continue;
            }
            out.push(FieldDef {
                name: f.name.name.clone(),
                wire_name,
                ty,
                optional: f.optional,
                doc: f.doc.clone(),
                deprecated: anns.deprecated,
                rules: anns.rules,
            });
        }
        out
    }
}
