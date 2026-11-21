//! The comparison itself. See the table in `docs/02-contract-language.md#evolving-contracts-safely`.

use std::collections::HashMap;

use weft_ir::{
    ErrorDef, FieldDef, Ir, ParamDef, Primitive, ResultDef, Rule, TypeBody, TypeDef, TypeRef,
};

use crate::direction::{directions, is_input, is_output, BOTH, OUTPUT};
use crate::{Change, Severity};

type RulePick = fn(&Rule) -> bool;

const DEPRECATE_FIRST: &str =
    "mark it `@deprecated` first, and remove it in the next major version";

pub fn diff(old: &Ir, new: &Ir) -> Vec<Change> {
    let mut dirs = directions(old);
    for (k, v) in directions(new) {
        *dirs.entry(k).or_insert(0) |= v;
    }
    let mut d = Differ {
        new,
        dirs,
        changes: Vec::new(),
    };
    d.types(old);
    d.errors(old);
    d.events(old);
    d.routes(old);
    d.rpc(old);
    d.graphql(old);
    d.sockets(old);
    d.changes
}

struct Differ<'a> {
    new: &'a Ir,
    dirs: HashMap<String, u8>,
    changes: Vec<Change>,
}

fn show(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Primitive { name } => match name {
            Primitive::String => "string",
            Primitive::Bool => "bool",
            Primitive::Int => "int",
            Primitive::Int32 => "int32",
            Primitive::Int64 => "int64",
            Primitive::Float => "float",
            Primitive::Float32 => "float32",
            Primitive::Float64 => "float64",
            Primitive::Decimal => "decimal",
            Primitive::Uuid => "uuid",
            Primitive::Timestamp => "timestamp",
            Primitive::Bytes => "bytes",
        }
        .to_string(),
        TypeRef::Named {
            package,
            name,
            args,
        } => {
            let n = format!("{package}.{name}");
            if args.is_empty() {
                n
            } else {
                format!(
                    "{n}<{}>",
                    args.iter().map(show).collect::<Vec<_>>().join(", ")
                )
            }
        }
        TypeRef::Param { name } => name.clone(),
        TypeRef::List { item } => format!("{}[]", show(item)),
        TypeRef::Map { key, value } => format!("map<{}, {}>", show(key), show(value)),
    }
}

/// How a change from `old` to `new` as a type is judged, or `None` if they are the same type.
fn type_change(old: &TypeRef, new: &TypeRef) -> Option<Severity> {
    use Primitive::*;
    match (old, new) {
        (TypeRef::Primitive { name: a }, TypeRef::Primitive { name: b }) => match (a, b) {
            _ if a == b => None,
            // Same representation under another name.
            (Int, Int64) | (Int64, Int) | (Float, Float64) | (Float64, Float) => None,
            // Wider: still valid, but a receiver may not expect the larger range.
            (Int32, Int | Int64) | (Float32, Float | Float64) => Some(Severity::Risky),
            _ => Some(Severity::Breaking),
        },
        (
            TypeRef::Named {
                package: p1,
                name: n1,
                args: a1,
            },
            TypeRef::Named {
                package: p2,
                name: n2,
                args: a2,
            },
        ) => {
            if p1 != p2 || n1 != n2 || a1.len() != a2.len() {
                return Some(Severity::Breaking);
            }
            a1.iter()
                .zip(a2)
                .filter_map(|(a, b)| type_change(a, b))
                .max()
        }
        (TypeRef::List { item: a }, TypeRef::List { item: b }) => type_change(a, b),
        (TypeRef::Map { key: k1, value: v1 }, TypeRef::Map { key: k2, value: v2 }) => {
            if type_change(k1, k2).is_some() {
                Some(Severity::Breaking)
            } else {
                type_change(v1, v2)
            }
        }
        (TypeRef::Param { name: a }, TypeRef::Param { name: b }) if a == b => None,
        _ => Some(Severity::Breaking),
    }
}

fn find<'x, T>(items: &'x [T], name: &str, key: impl Fn(&T) -> &str) -> Option<&'x T> {
    items.iter().find(|i| key(i) == name)
}

impl Differ<'_> {
    fn add(
        &mut self,
        severity: Severity,
        location: impl Into<String>,
        message: impl Into<String>,
        hint: Option<&str>,
    ) {
        self.changes.push(Change {
            severity,
            location: location.into(),
            message: message.into(),
            hint: if severity == Severity::Breaking {
                hint.map(str::to_string)
            } else {
                None
            },
        });
    }

    fn dir(&self, ty: &str) -> u8 {
        self.dirs.get(ty).copied().unwrap_or(BOTH)
    }

    // ---- types ----

    fn types(&mut self, old: &Ir) {
        for o in &old.types {
            match find(&self.new.types, &o.name, |t| &t.name) {
                None => self.add(
                    Severity::Breaking,
                    &o.name,
                    format!("removed type `{}`", o.name),
                    Some("other packages may import it; keep it, or remove it in the next major version"),
                ),
                Some(n) => {
                    let n = n.clone();
                    self.type_def(o, &n);
                }
            }
        }
        for n in &self.new.types {
            if !n.inline && find(&old.types, &n.name, |t| &t.name).is_none() {
                let msg = format!("added type `{}`", n.name);
                self.changes.push(Change {
                    severity: Severity::Safe,
                    location: n.name.clone(),
                    message: msg,
                    hint: None,
                });
            }
        }
    }

    fn type_def(&mut self, o: &TypeDef, n: &TypeDef) {
        let name = &o.name;
        if o.params != n.params {
            self.add(
                Severity::Breaking,
                name,
                format!(
                    "type parameters of `{name}` changed from <{}> to <{}>",
                    o.params.join(", "),
                    n.params.join(", ")
                ),
                Some("add a new type instead"),
            );
            return;
        }
        match (&o.body, &n.body) {
            (TypeBody::Record { fields: of }, TypeBody::Record { fields: nf }) => {
                let dir = self.dir(name);
                self.fields(name, of, nf, dir);
            }
            (TypeBody::Enum { variants: ov }, TypeBody::Enum { variants: nv }) => {
                for v in ov {
                    if find(nv, &v.name, |x| &x.name).is_none() {
                        self.add(
                            Severity::Breaking,
                            format!("{name}.{}", v.name),
                            format!("removed value `{}` from enum `{name}`", v.name),
                            Some(DEPRECATE_FIRST),
                        );
                    }
                }
                for v in nv {
                    if find(ov, &v.name, |x| &x.name).is_none() {
                        self.add(
                            Severity::Risky,
                            format!("{name}.{}", v.name),
                            format!(
                                "added value `{}` to enum `{name}`; receivers may not handle it",
                                v.name
                            ),
                            None,
                        );
                    }
                }
            }
            _ => self.add(
                Severity::Breaking,
                name,
                format!("`{name}` changed between a record and an enum"),
                Some("add a new type instead"),
            ),
        }
        if o.deprecated.is_none() && n.deprecated.is_some() {
            self.add(
                Severity::Safe,
                name,
                format!("`{name}` is now deprecated"),
                None,
            );
        }
    }

    fn fields(&mut self, owner: &str, old: &[FieldDef], new: &[FieldDef], dir: u8) {
        for o in old {
            match find(new, &o.name, |f| &f.name) {
                None => self.add(
                    Severity::Breaking,
                    format!("{owner}.{}", o.name),
                    format!("removed field `{}` from `{owner}`", o.name),
                    Some(DEPRECATE_FIRST),
                ),
                Some(n) => self.field(owner, o, n, dir),
            }
        }
        for n in new {
            if find(old, &n.name, |f| &f.name).is_some() {
                continue;
            }
            let loc = format!("{owner}.{}", n.name);
            if n.optional {
                self.add(
                    Severity::Safe,
                    loc,
                    format!("added optional field `{}` to `{owner}`", n.name),
                    None,
                );
            } else if is_input(dir) {
                self.add(
                    Severity::Breaking,
                    loc,
                    format!(
                        "added required field `{}` to `{owner}`, which callers send",
                        n.name
                    ),
                    Some("make the new field optional, or add it in the next major version"),
                );
            } else {
                self.add(
                    Severity::Safe,
                    loc,
                    format!(
                        "added required field `{}` to `{owner}`; receivers will always get it",
                        n.name
                    ),
                    None,
                );
            }
        }
    }

    fn field(&mut self, owner: &str, o: &FieldDef, n: &FieldDef, dir: u8) {
        let loc = format!("{owner}.{}", o.name);
        if o.wire_name != n.wire_name {
            self.add(
                Severity::Breaking,
                &loc,
                format!(
                    "wire name of `{}` changed from `{}` to `{}`",
                    o.name, o.wire_name, n.wire_name
                ),
                Some("add a new field with the new name, and deprecate this one"),
            );
        }
        if let Some(sev) = type_change(&o.ty, &n.ty) {
            let widening = sev == Severity::Risky;
            self.add(
                sev,
                &loc,
                format!(
                    "type of `{}` {} from `{}` to `{}`",
                    o.name,
                    if widening { "widened" } else { "changed" },
                    show(&o.ty),
                    show(&n.ty)
                ),
                Some("add a new field with the new type, and deprecate this one"),
            );
        }
        match (o.optional, n.optional) {
            (false, true) if is_output(dir) => self.add(
                Severity::Breaking,
                &loc,
                format!(
                    "`{}` was required and is now optional; receivers may get nothing",
                    o.name
                ),
                Some("keep it required, or make the change in the next major version"),
            ),
            (false, true) => self.add(
                Severity::Safe,
                &loc,
                format!("`{}` is now optional", o.name),
                None,
            ),
            (true, false) if is_input(dir) => self.add(
                Severity::Breaking,
                &loc,
                format!(
                    "`{}` was optional and is now required; callers must send it",
                    o.name
                ),
                Some("keep it optional, or make the change in the next major version"),
            ),
            (true, false) => self.add(
                Severity::Safe,
                &loc,
                format!("`{}` is now required", o.name),
                None,
            ),
            _ => {}
        }
        if o.deprecated.is_none() && n.deprecated.is_some() {
            self.add(
                Severity::Safe,
                &loc,
                format!("`{}` is now deprecated", o.name),
                None,
            );
        }
        self.rules(&loc, &o.rules, &n.rules, dir);
    }

    fn rules(&mut self, loc: &str, old: &[Rule], new: &[Rule], dir: u8) {
        let num = |r: &Rule| match r {
            Rule::Min { value } | Rule::Max { value } => value.parse::<f64>().unwrap_or(0.0),
            _ => 0.0,
        };
        let (mut tighter, mut looser): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        let get = |rules: &[Rule], pick: RulePick| rules.iter().find(|r| pick(r)).cloned();

        let kinds: [(&str, RulePick); 5] = [
            ("@min", |r| matches!(r, Rule::Min { .. })),
            ("@max", |r| matches!(r, Rule::Max { .. })),
            ("@length", |r| matches!(r, Rule::Length { .. })),
            ("@pattern", |r| matches!(r, Rule::Pattern { .. })),
            ("@format", |r| matches!(r, Rule::Format { .. })),
        ];
        for (label, pick) in kinds {
            match (get(old, pick), get(new, pick)) {
                (None, None) => {}
                (None, Some(_)) => tighter.push(format!("added `{label}`")),
                (Some(_), None) => looser.push(format!("removed `{label}`")),
                (Some(a), Some(b)) if a == b => {}
                (Some(a), Some(b)) => match (&a, &b) {
                    (Rule::Min { .. }, _) if num(&b) > num(&a) => {
                        tighter.push("raised `@min`".into())
                    }
                    (Rule::Min { .. }, _) => looser.push("lowered `@min`".into()),
                    (Rule::Max { .. }, _) if num(&b) < num(&a) => {
                        tighter.push("lowered `@max`".into())
                    }
                    (Rule::Max { .. }, _) => looser.push("raised `@max`".into()),
                    (Rule::Length { min: a1, max: a2 }, Rule::Length { min: b1, max: b2 }) => {
                        if b1 > a1 || b2 < a2 {
                            tighter.push("narrowed `@length`".into());
                        }
                        if b1 < a1 || b2 > a2 {
                            looser.push("widened `@length`".into());
                        }
                    }
                    // Two patterns or formats: there is no way to tell which accepts more.
                    _ => tighter.push(format!("changed `{label}`")),
                },
            }
        }
        if !tighter.is_empty() {
            let msg = format!("validation tightened: {}", tighter.join(", "));
            if is_input(dir) {
                self.add(
                    Severity::Breaking,
                    loc,
                    msg,
                    Some("senders may already send values that are now rejected; loosen it, or change it in the next major version"),
                );
            } else {
                self.add(Severity::Safe, loc, msg, None);
            }
        }
        if !looser.is_empty() {
            let msg = format!("validation loosened: {}", looser.join(", "));
            if is_output(dir) {
                self.add(
                    Severity::Risky,
                    loc,
                    format!("{msg}; receivers may rely on the old rule"),
                    None,
                );
            } else {
                self.add(Severity::Safe, loc, msg, None);
            }
        }
    }

    // ---- errors ----

    fn errors(&mut self, old: &Ir) {
        for o in &old.errors {
            match find(&self.new.errors, &o.name, |e| &e.name) {
                None => self.add(
                    Severity::Breaking,
                    &o.name,
                    format!("removed error `{}`", o.name),
                    Some(DEPRECATE_FIRST),
                ),
                Some(n) => {
                    let n: ErrorDef = n.clone();
                    if o.status != n.status {
                        self.add(
                            Severity::Breaking,
                            &o.name,
                            format!(
                                "HTTP status of `{}` changed from {} to {}",
                                o.name, o.status, n.status
                            ),
                            Some("callers may match on the status; add a new error instead"),
                        );
                    }
                    self.fields(&o.name, &o.fields, &n.fields, OUTPUT);
                }
            }
        }
        for n in &self.new.errors {
            if find(&old.errors, &n.name, |e| &e.name).is_none() {
                let msg = format!("added error `{}`", n.name);
                self.changes.push(Change {
                    severity: Severity::Safe,
                    location: n.name.clone(),
                    message: msg,
                    hint: None,
                });
            }
        }
    }

    // ---- operations ----

    fn result(&mut self, loc: &str, o: &ResultDef, n: &ResultDef) {
        if let Some(sev) = type_change(&o.value, &n.value) {
            self.add(
                sev,
                loc,
                format!(
                    "result changed from `{}` to `{}`",
                    show(&o.value),
                    show(&n.value)
                ),
                Some("add a new operation instead"),
            );
        }
        match (o.optional, n.optional) {
            (false, true) => self.add(
                Severity::Breaking,
                loc,
                "the result may now be empty",
                Some("callers may not handle an empty result; add a new operation instead"),
            ),
            (true, false) => self.add(
                Severity::Safe,
                loc,
                "the result is no longer optional",
                None,
            ),
            _ => {}
        }
        if o.stream != n.stream {
            self.add(
                Severity::Breaking,
                loc,
                if n.stream {
                    "the result is now a stream"
                } else {
                    "the result is no longer a stream"
                },
                Some("add a new operation instead"),
            );
        }
        for e in &n.errors {
            if !o.errors.contains(e) {
                self.add(
                    Severity::Risky,
                    loc,
                    format!("callers may now get the error `{}`", e.name),
                    None,
                );
            }
        }
        for e in &o.errors {
            if !n.errors.contains(e) {
                self.add(
                    Severity::Safe,
                    loc,
                    format!("the error `{}` is no longer returned", e.name),
                    None,
                );
            }
        }
    }

    fn param_list(&mut self, loc: &str, old: &[ParamDef], new: &[ParamDef]) {
        for o in old {
            match find(new, &o.name, |p| &p.name) {
                None => self.add(
                    Severity::Breaking,
                    loc,
                    format!("removed parameter `{}`", o.name),
                    Some(DEPRECATE_FIRST),
                ),
                Some(n) => {
                    if let Some(sev) = type_change(&o.ty, &n.ty) {
                        self.add(
                            sev,
                            loc,
                            format!(
                                "type of parameter `{}` changed from `{}` to `{}`",
                                o.name,
                                show(&o.ty),
                                show(&n.ty)
                            ),
                            Some("add a new parameter instead"),
                        );
                    }
                    match (o.optional, n.optional) {
                        (true, false) => self.add(
                            Severity::Breaking,
                            loc,
                            format!("parameter `{}` is now required", o.name),
                            Some("keep it optional, or make the change in the next major version"),
                        ),
                        (false, true) => self.add(
                            Severity::Safe,
                            loc,
                            format!("parameter `{}` is now optional", o.name),
                            None,
                        ),
                        _ => {}
                    }
                }
            }
        }
        for n in new {
            if find(old, &n.name, |p| &p.name).is_none() {
                if n.optional {
                    self.add(
                        Severity::Safe,
                        loc,
                        format!("added optional parameter `{}`", n.name),
                        None,
                    );
                } else {
                    self.add(
                        Severity::Breaking,
                        loc,
                        format!("added required parameter `{}`", n.name),
                        Some("make it optional, or add it in the next major version"),
                    );
                }
            }
        }
    }

    /// The `query`, `body` or `headers` of a route, which may be missing on either side.
    fn route_part(&mut self, loc: &str, label: &str, o: &Option<TypeRef>, n: &Option<TypeRef>) {
        match (o, n) {
            (None, None) => {}
            (Some(_), None) => self.add(
                Severity::Breaking,
                loc,
                format!("removed `{label}`"),
                Some("callers may still send it; add a new route instead"),
            ),
            (None, Some(t)) => {
                let all_optional = label != "body"
                    && match t {
                        TypeRef::Named { name, .. } => {
                            match find(&self.new.types, name, |x| &x.name).map(|d| &d.body) {
                                Some(TypeBody::Record { fields }) => {
                                    fields.iter().all(|f| f.optional)
                                }
                                _ => false,
                            }
                        }
                        _ => false,
                    };
                if all_optional {
                    self.add(
                        Severity::Safe,
                        loc,
                        format!("added `{label}` with only optional fields"),
                        None,
                    );
                } else {
                    self.add(
                        Severity::Breaking,
                        loc,
                        format!("added `{label}`, which callers must now send"),
                        Some("make every field optional, or add it in the next major version"),
                    );
                }
            }
            (Some(a), Some(b)) => {
                if let Some(sev) = type_change(a, b) {
                    self.add(
                        sev,
                        loc,
                        format!("`{label}` changed from `{}` to `{}`", show(a), show(b)),
                        Some("add a new route instead"),
                    );
                }
            }
        }
    }

    fn routes(&mut self, old: &Ir) {
        for o in &old.routes {
            let loc = format!("route {}", o.name);
            let Some(n) = find(&self.new.routes, &o.name, |r| &r.name) else {
                self.add(
                    Severity::Breaking,
                    &loc,
                    format!("removed route `{}`", o.name),
                    Some(DEPRECATE_FIRST),
                );
                continue;
            };
            let n = n.clone();
            if o.method != n.method {
                self.add(
                    Severity::Breaking,
                    &loc,
                    format!(
                        "method changed from {} to {}",
                        format!("{:?}", o.method).to_uppercase(),
                        format!("{:?}", n.method).to_uppercase()
                    ),
                    Some("add a new route instead"),
                );
            }
            let (op, np) = (
                format!("{}{}", o.base.as_deref().unwrap_or(""), o.path),
                format!("{}{}", n.base.as_deref().unwrap_or(""), n.path),
            );
            if op != np {
                self.add(
                    Severity::Breaking,
                    &loc,
                    format!("path changed from `{op}` to `{np}`"),
                    Some("add a new route, and deprecate this one"),
                );
            }
            self.param_list(&loc, &o.path_params, &n.path_params);
            self.route_part(&loc, "query", &o.query, &n.query);
            self.route_part(&loc, "body", &o.body, &n.body);
            self.route_part(&loc, "headers", &o.headers, &n.headers);
            self.result(&loc, &o.result, &n.result);
            if o.deprecated.is_none() && n.deprecated.is_some() {
                self.add(
                    Severity::Safe,
                    &loc,
                    format!("route `{}` is now deprecated", o.name),
                    None,
                );
            }
        }
        for n in &self.new.routes {
            if find(&old.routes, &n.name, |r| &r.name).is_none() {
                let msg = format!("added route `{}`", n.name);
                self.changes.push(Change {
                    severity: Severity::Safe,
                    location: format!("route {}", n.name),
                    message: msg,
                    hint: None,
                });
            }
        }
    }

    fn events(&mut self, old: &Ir) {
        for o in &old.events {
            let loc = format!("event {}", o.name);
            let Some(n) = find(&self.new.events, &o.name, |e| &e.name) else {
                self.add(
                    Severity::Breaking,
                    &loc,
                    format!("removed event `{}`", o.name),
                    Some(DEPRECATE_FIRST),
                );
                continue;
            };
            let n = n.clone();
            if let Some(sev) = type_change(&o.payload, &n.payload) {
                self.add(
                    sev,
                    &loc,
                    format!(
                        "payload changed from `{}` to `{}`",
                        show(&o.payload),
                        show(&n.payload)
                    ),
                    Some("add a new event instead"),
                );
            }
            if o.key != n.key {
                self.add(
                    Severity::Breaking,
                    &loc,
                    format!(
                        "routing key changed from `{}` to `{}`; ordering and partitioning change",
                        o.key.join("."),
                        n.key.join(".")
                    ),
                    Some("add a new event with the new key instead"),
                );
            }
            match (&o.headers, &n.headers) {
                (None, None) => {}
                (None, Some(_)) => self.add(Severity::Safe, &loc, "added headers", None),
                (Some(_), None) => self.add(
                    Severity::Breaking,
                    &loc,
                    "removed headers",
                    Some("receivers may read them; deprecate first"),
                ),
                (Some(a), Some(b)) => {
                    if let Some(sev) = type_change(a, b) {
                        self.add(
                            sev,
                            &loc,
                            "headers changed type",
                            Some("add new headers instead"),
                        );
                    }
                }
            }
            if o.deprecated.is_none() && n.deprecated.is_some() {
                self.add(
                    Severity::Safe,
                    &loc,
                    format!("event `{}` is now deprecated", o.name),
                    None,
                );
            }
        }
        for n in &self.new.events {
            if find(&old.events, &n.name, |e| &e.name).is_none() {
                let msg = format!("added event `{}`", n.name);
                self.changes.push(Change {
                    severity: Severity::Safe,
                    location: format!("event {}", n.name),
                    message: msg,
                    hint: None,
                });
            }
        }
    }

    fn rpc(&mut self, old: &Ir) {
        for os in &old.rpc_services {
            let Some(ns) = find(&self.new.rpc_services, &os.name, |s| &s.name) else {
                self.add(
                    Severity::Breaking,
                    format!("rpc {}", os.name),
                    format!("removed rpc service `{}`", os.name),
                    Some(DEPRECATE_FIRST),
                );
                continue;
            };
            let ns = ns.clone();
            for o in &os.methods {
                let loc = format!("rpc {}.{}", os.name, o.name);
                let Some(n) = find(&ns.methods, &o.name, |m| &m.name) else {
                    self.add(
                        Severity::Breaking,
                        &loc,
                        format!("removed method `{}`", o.name),
                        Some(DEPRECATE_FIRST),
                    );
                    continue;
                };
                if let Some(sev) = type_change(&o.request, &n.request) {
                    self.add(
                        sev,
                        &loc,
                        format!(
                            "request changed from `{}` to `{}`",
                            show(&o.request),
                            show(&n.request)
                        ),
                        Some("add a new method instead"),
                    );
                }
                if o.client_streaming != n.client_streaming {
                    self.add(
                        Severity::Breaking,
                        &loc,
                        "client streaming changed",
                        Some("add a new method instead"),
                    );
                }
                self.result(&loc, &o.result, &n.result);
            }
            for n in &ns.methods {
                if find(&os.methods, &n.name, |m| &m.name).is_none() {
                    let msg = format!("added method `{}`", n.name);
                    self.changes.push(Change {
                        severity: Severity::Safe,
                        location: format!("rpc {}.{}", os.name, n.name),
                        message: msg,
                        hint: None,
                    });
                }
            }
        }
        for ns in &self.new.rpc_services {
            if find(&old.rpc_services, &ns.name, |s| &s.name).is_none() {
                let msg = format!("added rpc service `{}`", ns.name);
                self.changes.push(Change {
                    severity: Severity::Safe,
                    location: format!("rpc {}", ns.name),
                    message: msg,
                    hint: None,
                });
            }
        }
    }

    fn graphql(&mut self, old: &Ir) {
        for o in &old.graphql {
            let loc = format!("{} {}", format!("{:?}", o.kind).to_lowercase(), o.name);
            let Some(n) = self
                .new
                .graphql
                .iter()
                .find(|f| f.kind == o.kind && f.name == o.name)
            else {
                self.add(
                    Severity::Breaking,
                    &loc,
                    format!("removed `{}`", o.name),
                    Some(DEPRECATE_FIRST),
                );
                continue;
            };
            let n = n.clone();
            self.param_list(&loc, &o.args, &n.args);
            self.result(&loc, &o.result, &n.result);
        }
        for n in &self.new.graphql {
            if !old
                .graphql
                .iter()
                .any(|f| f.kind == n.kind && f.name == n.name)
            {
                let msg = format!("added `{}`", n.name);
                self.changes.push(Change {
                    severity: Severity::Safe,
                    location: format!("{} {}", format!("{:?}", n.kind).to_lowercase(), n.name),
                    message: msg,
                    hint: None,
                });
            }
        }
    }

    fn sockets(&mut self, old: &Ir) {
        for os in &old.sockets {
            let Some(ns) = find(&self.new.sockets, &os.name, |s| &s.name) else {
                self.add(
                    Severity::Breaking,
                    format!("socket {}", os.name),
                    format!("removed socket `{}`", os.name),
                    Some(DEPRECATE_FIRST),
                );
                continue;
            };
            let ns = ns.clone();
            for o in &os.events {
                let loc = format!("socket {}.{}", os.name, o.name);
                let Some(n) = find(&ns.events, &o.name, |e| &e.name) else {
                    self.add(
                        Severity::Breaking,
                        &loc,
                        format!("removed event `{}`", o.name),
                        Some(DEPRECATE_FIRST),
                    );
                    continue;
                };
                if o.side != n.side {
                    self.add(
                        Severity::Breaking,
                        &loc,
                        "the sending side changed",
                        Some("add a new event instead"),
                    );
                }
                if let Some(sev) = type_change(&o.payload, &n.payload) {
                    self.add(
                        sev,
                        &loc,
                        format!(
                            "payload changed from `{}` to `{}`",
                            show(&o.payload),
                            show(&n.payload)
                        ),
                        Some("add a new event instead"),
                    );
                }
            }
            for n in &ns.events {
                if find(&os.events, &n.name, |e| &e.name).is_none() {
                    let msg = format!("added event `{}`", n.name);
                    self.changes.push(Change {
                        severity: Severity::Safe,
                        location: format!("socket {}.{}", os.name, n.name),
                        message: msg,
                        hint: None,
                    });
                }
            }
        }
        for ns in &self.new.sockets {
            if find(&old.sockets, &ns.name, |s| &s.name).is_none() {
                let msg = format!("added socket `{}`", ns.name);
                self.changes.push(Change {
                    severity: Severity::Safe,
                    location: format!("socket {}", ns.name),
                    message: msg,
                    hint: None,
                });
            }
        }
    }
}
