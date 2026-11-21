//! Name lookup and type resolution, including generated names for inline types.

use weft_ir::{ErrorRef, Primitive, TypeBody, TypeDef, TypeRef, BUILTIN_ERRORS, BUILTIN_PACKAGE};
use weft_syntax::ast::{self as a, TypeKind};
use weft_syntax::Span;

use crate::error::suggest;
use crate::lower::Lower;
use crate::{Entity, EntityKind};

pub(crate) const PRIMITIVES: [(&str, Primitive); 12] = [
    ("string", Primitive::String),
    ("bool", Primitive::Bool),
    ("int", Primitive::Int),
    ("int32", Primitive::Int32),
    ("int64", Primitive::Int64),
    ("float", Primitive::Float),
    ("float32", Primitive::Float32),
    ("float64", Primitive::Float64),
    ("decimal", Primitive::Decimal),
    ("uuid", Primitive::Uuid),
    ("timestamp", Primitive::Timestamp),
    ("bytes", Primitive::Bytes),
];

pub(crate) fn prim_from(name: &str) -> Option<Primitive> {
    PRIMITIVES.iter().find(|(n, _)| *n == name).map(|(_, p)| *p)
}

fn prim_name(p: Primitive) -> &'static str {
    PRIMITIVES.iter().find(|(_, q)| *q == p).unwrap().0
}

fn plain(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Primitive { name } => prim_name(*name).to_string(),
        TypeRef::Named { name, args, .. } if args.is_empty() => name.clone(),
        TypeRef::Named { name, args, .. } => {
            let inner: Vec<String> = args.iter().map(plain).collect();
            format!("{name}<{}>", inner.join(", "))
        }
        TypeRef::Param { name } => name.clone(),
        TypeRef::List { item } => format!("{}[]", plain(item)),
        TypeRef::Map { key, value } => format!("map<{}, {}>", plain(key), plain(value)),
    }
}

/// A type as the author would write it, in backticks, for messages.
pub(crate) fn show(ty: &TypeRef) -> String {
    format!("`{}`", plain(ty))
}

/// What a name refers to.
pub(crate) enum Found {
    Primitive(Primitive),
    Map,
    Param(String),
    Item {
        package: String,
        name: String,
        entity: Entity,
    },
    BuiltinError(String),
}

impl Lower<'_> {
    pub fn entity(&self, package: &str, name: &str) -> Option<&Entity> {
        self.indexes.get(package)?.entities.get(name)
    }

    /// The definition of a record or enum already lowered, in this package or an imported one.
    pub fn type_def(&self, package: &str, name: &str) -> Option<&TypeDef> {
        let ir = if package == self.package {
            &self.ir
        } else {
            self.done.get(package)?
        };
        ir.types.iter().find(|t| t.name == name)
    }

    pub fn lookup(&mut self, path: &a::Path) -> Option<Found> {
        let segs = &path.segments;
        match segs.as_slice() {
            [one] => {
                let name = one.name.as_str();
                if self.generics.iter().any(|g| g == name) {
                    return Some(Found::Param(name.to_string()));
                }
                if let Some(p) = prim_from(name) {
                    return Some(Found::Primitive(p));
                }
                if name == "map" {
                    return Some(Found::Map);
                }
                if let Some(e) = self.entity(self.package, name) {
                    return Some(Found::Item {
                        package: self.package.to_string(),
                        name: name.to_string(),
                        entity: e.clone(),
                    });
                }
                if BUILTIN_ERRORS.iter().any(|(n, _)| *n == name) {
                    return Some(Found::BuiltinError(name.to_string()));
                }
                let mut candidates: Vec<&str> = PRIMITIVES.iter().map(|(n, _)| *n).collect();
                candidates.extend(
                    self.indexes[self.package]
                        .entities
                        .keys()
                        .map(String::as_str),
                );
                candidates.extend(self.generics.iter().map(String::as_str));
                let help =
                    suggest(name, candidates.into_iter()).map(|s| format!("did you mean `{s}`?"));
                self.error(
                    &one.span,
                    format!("unknown type `{name}`"),
                    "not declared",
                    help,
                );
                None
            }
            [pkg, name] => {
                let (p, n) = (pkg.name.as_str(), name.name.as_str());
                if p == BUILTIN_PACKAGE {
                    if BUILTIN_ERRORS.iter().any(|(b, _)| *b == n) {
                        return Some(Found::BuiltinError(n.to_string()));
                    }
                    self.error(
                        &path.span,
                        format!("`{n}` is not a built-in error"),
                        "unknown",
                        None,
                    );
                    return None;
                }
                if p == self.package {
                    self.error(
                        &path.span,
                        format!("`{p}` is this package"),
                        "write the name without the package",
                        Some(format!("use `{n}`")),
                    );
                    return None;
                }
                if !self.imports.iter().any(|i| i == p) {
                    let known = self.indexes.contains_key(p);
                    self.error(
                        &pkg.span,
                        format!("package `{p}` is not imported"),
                        "not imported in this file",
                        Some(if known {
                            format!("add `import {p}` after the `package` line")
                        } else {
                            format!("no package `{p}` exists")
                        }),
                    );
                    return None;
                }
                match self.entity(p, n) {
                    Some(e) => Some(Found::Item {
                        package: p.to_string(),
                        name: n.to_string(),
                        entity: e.clone(),
                    }),
                    None => {
                        let help = self.indexes.get(p).and_then(|i| {
                            suggest(n, i.entities.keys().map(String::as_str))
                                .map(|s| format!("did you mean `{p}.{s}`?"))
                        });
                        self.error(
                            &name.span,
                            format!("package `{p}` has no type `{n}`"),
                            "not declared there",
                            help,
                        );
                        None
                    }
                }
            }
            _ => {
                self.error(
                    &path.span,
                    "a name has at most two parts: `package.Name`",
                    "too many parts",
                    None,
                );
                None
            }
        }
    }

    /// Resolve a type that may be used for a field, parameter or payload. `ctx` is the name a generated type
    /// would get if this type is written inline.
    pub fn resolve_type(&mut self, ty: &a::TypeExpr, ctx: &str) -> Option<TypeRef> {
        match &ty.kind {
            TypeKind::Named { path, args } => {
                let found = self.lookup(path)?;
                self.type_from_found(found, path, args, &ty.span, ctx)
            }
            TypeKind::List(inner) => Some(TypeRef::List {
                item: Box::new(self.resolve_type(inner, ctx)?),
            }),
            TypeKind::Inline(fields) => self.inline_record(ctx, fields, &ty.span),
            TypeKind::Optional(_) | TypeKind::Union(_) => {
                self.error(
                    &ty.span,
                    "`?` after a type and `|` are only for results",
                    "not allowed here",
                    Some("write an optional field as `name?: T`".into()),
                );
                None
            }
        }
    }

    fn type_from_found(
        &mut self,
        found: Found,
        path: &a::Path,
        args: &[a::TypeExpr],
        span: &Span,
        ctx: &str,
    ) -> Option<TypeRef> {
        let no_args = |this: &mut Self, what: &str| {
            if !args.is_empty() {
                this.error(
                    span,
                    format!("{what} takes no type arguments"),
                    "remove the `<...>`",
                    None,
                );
                return false;
            }
            true
        };
        match found {
            Found::Primitive(p) => no_args(self, &format!("`{}`", path.text()))
                .then_some(TypeRef::Primitive { name: p }),
            Found::Param(name) => {
                no_args(self, &format!("`{name}`")).then_some(TypeRef::Param { name })
            }
            Found::Map => {
                if args.len() != 2 {
                    self.error(
                        span,
                        "`map` takes a key type and a value type",
                        "expected `map<K, V>`",
                        None,
                    );
                    return None;
                }
                let key = self.resolve_type(&args[0], ctx);
                let value = self.resolve_type(&args[1], ctx);
                let (key, value) = (key?, value?);
                if !self.is_key_type(&key) {
                    self.error(
                        &args[0].span,
                        format!("{} cannot be a map key", show(&key)),
                        "keys must be strings, uuids or enums",
                        None,
                    );
                    return None;
                }
                Some(TypeRef::Map {
                    key: Box::new(key),
                    value: Box::new(value),
                })
            }
            Found::BuiltinError(name) => {
                self.error(
                    span,
                    format!("`{name}` is an error, not a type"),
                    "errors are listed after a result",
                    Some(format!("write `-> Value | {name}`")),
                );
                None
            }
            Found::Item {
                package,
                name,
                entity,
            } => {
                if entity.kind == EntityKind::Error {
                    self.error(
                        span,
                        format!("`{name}` is an error, not a type"),
                        "errors are listed after a result",
                        Some(format!("write `-> Value | {name}`")),
                    );
                    return None;
                }
                if args.len() != entity.arity {
                    let msg = if entity.arity == 0 {
                        format!("`{name}` is not generic")
                    } else {
                        format!(
                            "`{name}` takes {} type argument(s), not {}",
                            entity.arity,
                            args.len()
                        )
                    };
                    self.error(span, msg, "wrong number of type arguments", None);
                    return None;
                }
                let mut resolved = Vec::new();
                for arg in args {
                    resolved.push(self.resolve_type(arg, ctx));
                }
                let args: Option<Vec<TypeRef>> = resolved.into_iter().collect();
                Some(TypeRef::Named {
                    package,
                    name,
                    args: args?,
                })
            }
        }
    }

    fn is_key_type(&self, ty: &TypeRef) -> bool {
        match ty {
            TypeRef::Primitive { name } => matches!(name, Primitive::String | Primitive::Uuid),
            TypeRef::Named { package, name, .. } => self
                .entity(package, name)
                .is_some_and(|e| e.kind == EntityKind::Enum),
            _ => false,
        }
    }

    /// An inline record becomes a generated type named `name`.
    pub fn inline_record(
        &mut self,
        name: &str,
        fields: &[a::Field],
        span: &Span,
    ) -> Option<TypeRef> {
        if !self.generics.is_empty() {
            self.error(
                span,
                "inline types are not allowed inside a generic type",
                "declare a named type instead",
                None,
            );
            return None;
        }
        if !self.taken.insert(name.to_string()) {
            self.error(
                span,
                format!("the generated type name `{name}` is already used"),
                "this inline type would be called the same",
                Some(
                    "declare this type with a name of your own, or rename what it belongs to"
                        .into(),
                ),
            );
            return None;
        }
        let slot = self.ir.types.len();
        self.ir.types.push(TypeDef {
            name: name.to_string(),
            doc: vec![],
            params: vec![],
            inline: true,
            deprecated: None,
            body: TypeBody::Record { fields: vec![] },
        });
        let lowered = self.lower_fields(fields, name, "type");
        self.ir.types[slot].body = TypeBody::Record { fields: lowered };
        Some(TypeRef::Named {
            package: self.package.to_string(),
            name: name.to_string(),
            args: vec![],
        })
    }

    /// Whether `ty` refers to a record (as opposed to an enum, a scalar, a list or a map). Generated inline types
    /// are not in the name index, so their definitions are checked too.
    pub fn is_record(&self, ty: &TypeRef) -> bool {
        let TypeRef::Named { package, name, .. } = ty else {
            return false;
        };
        match self.entity(package, name) {
            Some(e) => e.kind == EntityKind::Record,
            None => self.record_fields(ty).is_some(),
        }
    }

    pub fn require_record(&mut self, ty: &TypeRef, span: &Span, what: &str) -> bool {
        if self.is_record(ty) {
            return true;
        }
        self.error(
            span,
            format!("{what} must be a record type, not {}", show(ty)),
            "not a record",
            Some("declare a `type` with fields, or write the fields inline: `{ id: uuid }`".into()),
        );
        false
    }

    /// The fields of a record, when its definition is known.
    pub fn record_fields(&self, ty: &TypeRef) -> Option<&[weft_ir::FieldDef]> {
        let TypeRef::Named { package, name, .. } = ty else {
            return None;
        };
        match &self.type_def(package, name)?.body {
            TypeBody::Record { fields } => Some(fields),
            TypeBody::Enum { .. } => None,
        }
    }

    /// Report fields that are not strings, for headers.
    pub fn require_string_fields(&mut self, ty: &TypeRef, span: &Span, what: &str) {
        let bad: Vec<String> = match self.record_fields(ty) {
            Some(fields) => fields
                .iter()
                .filter(|f| {
                    !matches!(
                        &f.ty,
                        TypeRef::Primitive {
                            name: Primitive::String
                        }
                    )
                })
                .map(|f| f.name.clone())
                .collect(),
            None => return,
        };
        if !bad.is_empty() {
            self.error(
                span,
                format!(
                    "{what} can only hold `string` fields; not: {}",
                    bad.join(", ")
                ),
                "headers travel as text",
                None,
            );
        }
    }

    /// Resolve one error named in a result.
    pub fn resolve_error(&mut self, ty: &a::TypeExpr) -> Option<ErrorRef> {
        let TypeKind::Named { path, args } = &ty.kind else {
            self.error(
                &ty.span,
                "only errors can follow the result type",
                "not an error",
                Some(
                    "errors are the built-in ones (`NotFound`, ...) or declared with `error`"
                        .into(),
                ),
            );
            return None;
        };
        if !args.is_empty() {
            self.error(
                &ty.span,
                "errors take no type arguments",
                "remove the `<...>`",
                None,
            );
            return None;
        }
        match self.lookup(path)? {
            Found::BuiltinError(name) => Some(ErrorRef {
                package: BUILTIN_PACKAGE.to_string(),
                name,
            }),
            Found::Item {
                package,
                name,
                entity,
            } if entity.kind == EntityKind::Error => Some(ErrorRef { package, name }),
            Found::Item { name, .. } => {
                self.error(
                    &ty.span,
                    format!("`{name}` is not an error"),
                    "only errors can follow the result type",
                    Some("the result value comes first: `-> Value | SomeError`".into()),
                );
                None
            }
            _ => {
                self.error(
                    &ty.span,
                    "only errors can follow the result type",
                    "not an error",
                    None,
                );
                None
            }
        }
    }
}
