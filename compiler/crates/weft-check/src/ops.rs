//! Events, HTTP routes, RPC services, GraphQL fields and sockets.

use std::collections::HashSet;

use weft_ir::{
    EventDef, FieldDef, GraphqlFieldDef, GraphqlKind, HttpMethod, ParamDef, Primitive, ResultDef,
    RouteDef, RpcMethodDef, RpcServiceDef, Side, SocketDef, SocketEventDef, TypeBody, TypeDef,
    TypeRef,
};
use weft_syntax::ast::{self as a, DeclKind, TypeKind};
use weft_syntax::Span;

use crate::annotations::Target;
use crate::error::{pascal, suggest};
use crate::lower::Lower;
use crate::resolve::show;
use crate::EntityKind;

const METHODS: [(&str, HttpMethod); 7] = [
    ("GET", HttpMethod::Get),
    ("POST", HttpMethod::Post),
    ("PUT", HttpMethod::Put),
    ("PATCH", HttpMethod::Patch),
    ("DELETE", HttpMethod::Delete),
    ("HEAD", HttpMethod::Head),
    ("OPTIONS", HttpMethod::Options),
];

impl Lower<'_> {
    pub fn lower_operation_decl(&mut self, d: &a::Decl) {
        match &d.kind {
            DeclKind::Event(e) => self.lower_event(d, e),
            DeclKind::Http(h) => self.lower_http(h),
            DeclKind::Rpc(r) => self.lower_rpc(r),
            DeclKind::Graphql(g) => self.lower_graphql(g),
            DeclKind::Socket(s) => self.lower_socket(s),
            DeclKind::Enum(_) | DeclKind::Type(_) | DeclKind::Error(_) => {}
        }
    }

    // ---- results ----

    /// `-> [stream] Value[?] | Error | ...`
    fn lower_result(
        &mut self,
        r: &a::ResultType,
        ctx: &str,
        allow_stream: bool,
    ) -> Option<ResultDef> {
        if r.stream && !allow_stream {
            self.error(
                &r.span,
                "`stream` results are only for `rpc` methods",
                "not allowed here",
                Some("GraphQL uses `subscription`; messaging uses events".into()),
            );
        }
        let members: Vec<&a::TypeExpr> = match &r.ty.kind {
            TypeKind::Union(v) => v.iter().collect(),
            _ => vec![&r.ty],
        };
        let (first, optional) = match &members[0].kind {
            TypeKind::Optional(inner) => (&**inner, true),
            _ => (members[0], false),
        };
        let value = self.resolve_type(first, &format!("{ctx}Result"));
        let mut errors = Vec::new();
        for m in &members[1..] {
            if matches!(m.kind, TypeKind::Optional(_)) {
                self.error(
                    &m.span,
                    "only the result value can be optional",
                    "remove the `?`",
                    None,
                );
                continue;
            }
            if let Some(e) = self.resolve_error(m) {
                if errors.contains(&e) {
                    self.error(
                        &m.span,
                        format!("`{}` is listed twice", e.name),
                        "duplicate",
                        None,
                    );
                } else {
                    errors.push(e);
                }
            }
        }
        Some(ResultDef {
            value: value?,
            optional,
            stream: r.stream,
            errors,
        })
    }

    // ---- events ----

    fn lower_event(&mut self, d: &a::Decl, e: &a::EventDecl) {
        let name = e.name.text();
        let anns = self.annotations(&e.annotations, Target::Event, None);
        if !self.event_names.insert(name.clone()) {
            self.error(
                &e.name.span,
                format!("event `{name}` is declared twice"),
                "duplicate event",
                None,
            );
            return;
        }
        let ctx = pascal(&name);
        let payload = self.resolve_type(&e.ty, &ctx);
        if let Some(p) = &payload {
            if !self.require_record(p, &e.ty.span, "an event payload") {
                return;
            }
        }

        let mut key: Vec<String> = Vec::new();
        let mut headers = None;
        let mut seen_key = false;
        for opt in &e.options {
            match opt {
                a::EventOption::Key { path, span } => {
                    if std::mem::replace(&mut seen_key, true) {
                        self.error(span, "an event has one `key`", "duplicate", None);
                        continue;
                    }
                    if let Some(p) = &payload {
                        key = self.check_key(p, path).unwrap_or_default();
                    }
                }
                a::EventOption::Headers { ty, span } => {
                    if headers.is_some() {
                        self.error(span, "an event has one `headers`", "duplicate", None);
                        continue;
                    }
                    let h = self.resolve_type(ty, &format!("{ctx}Headers"));
                    if let Some(h) = &h {
                        if self.require_record(h, &ty.span, "`headers`") {
                            self.require_string_fields(h, &ty.span, "`headers`");
                        }
                    }
                    headers = h;
                }
            }
        }
        if let Some(payload) = payload {
            self.ir.events.push(EventDef {
                name,
                doc: d.doc.clone(),
                payload,
                key,
                headers,
                deprecated: anns.deprecated,
            });
        }
    }

    /// Check an event's `key: a.b` against the payload record.
    fn check_key(&mut self, payload: &TypeRef, path: &a::Path) -> Option<Vec<String>> {
        let mut current = payload.clone();
        let mut out = Vec::new();
        let last = path.segments.len() - 1;
        for (i, seg) in path.segments.iter().enumerate() {
            let fields: Vec<FieldDef> = self.record_fields(&current)?.to_vec();
            let Some(f) = fields.iter().find(|f| f.name == seg.name) else {
                let help = suggest(&seg.name, fields.iter().map(|f| f.name.as_str()))
                    .map(|s| format!("did you mean `{s}`?"));
                self.error(
                    &seg.span,
                    format!("{} has no field `{}`", show(&current), seg.name),
                    "no such field",
                    help,
                );
                return None;
            };
            if f.optional {
                self.error(
                    &seg.span,
                    format!("`{}` is optional, so it cannot be part of a key", seg.name),
                    "may be missing",
                    None,
                );
                return None;
            }
            out.push(seg.name.clone());
            if i == last {
                let ok = match &f.ty {
                    TypeRef::Primitive { name } => matches!(
                        name,
                        Primitive::String
                            | Primitive::Int
                            | Primitive::Int32
                            | Primitive::Int64
                            | Primitive::Uuid
                    ),
                    TypeRef::Named { package, name, .. } => self
                        .entity(package, name)
                        .is_some_and(|e| e.kind == EntityKind::Enum),
                    _ => false,
                };
                if !ok {
                    self.error(
                        &seg.span,
                        format!(
                            "a key must be a string, whole number, uuid or enum, not {}",
                            show(&f.ty)
                        ),
                        "not usable as a key",
                        None,
                    );
                    return None;
                }
            } else {
                current = f.ty.clone();
            }
        }
        Some(out)
    }

    // ---- http ----

    fn lower_http(&mut self, h: &a::HttpBlock) {
        let base = h.base.as_ref().map(|(b, _)| b.clone());
        if let Some((b, span)) = &h.base {
            if b.contains('{') {
                self.error(
                    span,
                    "a base path cannot have parameters",
                    "remove the `{...}`",
                    None,
                );
            }
        }
        for route in &h.routes {
            self.lower_route(route, base.as_deref());
        }
    }

    fn lower_route(&mut self, r: &a::HttpRoute, base: Option<&str>) {
        let anns = self.annotations(&r.annotations, Target::Operation, None);
        let mut ok = true;

        let method = METHODS
            .iter()
            .find(|(n, _)| *n == r.method.name)
            .map(|(_, m)| *m);
        if method.is_none() {
            let help = suggest(
                &r.method.name.to_uppercase(),
                METHODS.iter().map(|(n, _)| *n),
            )
            .map(|s| format!("did you mean `{s}`?"));
            self.error(
                &r.method.span,
                format!("`{}` is not an HTTP method", r.method.name),
                "unknown method",
                help.or(Some(
                    "methods are GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS".into(),
                )),
            );
            ok = false;
        }

        let (path, path_span) = (&r.path.0, &r.path.1);
        let template = self.parse_path(path, path_span);
        if template.is_none() {
            ok = false;
        }
        let template = template.unwrap_or_default();

        if !self.route_names.insert(r.name.name.clone()) {
            self.error(
                &r.name.span,
                format!("route `{}` is declared twice", r.name.name),
                "duplicate route name",
                Some("route names become function names in generated clients".into()),
            );
            ok = false;
        }
        if let Some(m) = method {
            let shape = format!(
                "{m:?} {}{}",
                base.unwrap_or(""),
                template.iter().fold(String::new(), |mut s, seg| {
                    match seg {
                        Seg::Lit(l) => s.push_str(l),
                        Seg::Param(_) => s.push_str("{}"),
                    }
                    s
                })
            );
            if !self.route_shapes.insert(shape) {
                self.error(
                    path_span,
                    format!(
                        "another route already handles {} {}{path}",
                        r.method.name,
                        base.unwrap_or("")
                    ),
                    "duplicate route",
                    None,
                );
                ok = false;
            }
        }

        let ctx = pascal(&r.name.name);
        let mut path_decls: Vec<(&a::Param, TypeRef)> = Vec::new();
        let (mut query, mut body, mut headers) = (None, None, None);
        for p in &r.params {
            let Some(pname) = &p.name else {
                self.error(
                    &p.span,
                    "a route parameter needs a name",
                    "write `name: Type`",
                    None,
                );
                ok = false;
                continue;
            };
            match pname.name.as_str() {
                "body" | "query" | "headers" => {
                    if p.optional {
                        self.error(
                            &p.span,
                            format!("`{}` cannot be optional", pname.name),
                            "remove the `?`",
                            Some("make the fields inside it optional instead".into()),
                        );
                        ok = false;
                    }
                    let ty = self.resolve_type(&p.ty, &format!("{ctx}{}", pascal(&pname.name)));
                    let Some(ty) = ty else {
                        ok = false;
                        continue;
                    };
                    match pname.name.as_str() {
                        "body" => {
                            if matches!(method, Some(HttpMethod::Get | HttpMethod::Head)) {
                                self.error(
                                    &p.span,
                                    format!("{} routes cannot have a body", r.method.name),
                                    "remove `body`",
                                    None,
                                );
                                ok = false;
                            }
                            body = Some(ty);
                        }
                        "query" => {
                            ok &= self.require_record(&ty, &p.ty.span, "`query`");
                            query = Some(ty);
                        }
                        _ => {
                            if self.require_record(&ty, &p.ty.span, "`headers`") {
                                self.require_string_fields(&ty, &p.ty.span, "`headers`");
                            } else {
                                ok = false;
                            }
                            headers = Some(ty);
                        }
                    }
                }
                _ => {
                    let ty = self.resolve_type(&p.ty, &format!("{ctx}{}", pascal(&pname.name)));
                    match ty {
                        Some(ty) => path_decls.push((p, ty)),
                        None => ok = false,
                    }
                }
            }
        }

        // Match path parameters in path order.
        let mut path_params = Vec::new();
        let mut used = HashSet::new();
        for seg in &template {
            let Seg::Param(name) = seg else { continue };
            let Some((p, ty)) = path_decls
                .iter()
                .find(|(p, _)| p.name.as_ref().unwrap().name == *name)
            else {
                self.error(
                    path_span,
                    format!("path parameter `{{{name}}}` is not declared"),
                    "no matching parameter",
                    Some(format!(
                        "add `{name}: uuid` (or another type) to the parameters"
                    )),
                );
                ok = false;
                continue;
            };
            if !used.insert(name.clone()) {
                self.error(
                    path_span,
                    format!("`{{{name}}}` appears twice in the path"),
                    "duplicate",
                    None,
                );
                ok = false;
                continue;
            }
            let scalar = match ty {
                TypeRef::Primitive { name } => !matches!(name, Primitive::Bytes),
                TypeRef::Named { package, name, .. } => self
                    .entity(package, name)
                    .is_some_and(|e| e.kind == EntityKind::Enum),
                _ => false,
            };
            if p.optional || !scalar {
                self.error(
                    &p.span,
                    format!("path parameter `{name}` must be a required scalar"),
                    if p.optional {
                        "remove the `?`"
                    } else {
                        "not usable in a path"
                    },
                    None,
                );
                ok = false;
                continue;
            }
            path_params.push(ParamDef {
                name: name.clone(),
                ty: ty.clone(),
                optional: false,
            });
        }
        for (p, _) in &path_decls {
            let n = &p.name.as_ref().unwrap().name;
            if !used.contains(n) {
                self.error(
                    &p.span,
                    format!("`{n}` is not in the path `{path}`"),
                    "not in the path",
                    Some(format!(
                        "query parameters go in `query: {{ {n}: ... }}`; the body goes in `body: ...`"
                    )),
                );
                ok = false;
            }
        }

        let result = self.lower_result(&r.result, &ctx, false);
        if let (true, Some(method), Some(result)) = (ok, method, result) {
            self.ir.routes.push(RouteDef {
                name: r.name.name.clone(),
                doc: r.doc.clone(),
                method,
                base: base.map(str::to_string),
                path: path.clone(),
                path_params,
                query,
                body,
                headers,
                result,
                deprecated: anns.deprecated,
            });
        }
    }

    fn parse_path(&mut self, path: &str, span: &Span) -> Option<Vec<Seg>> {
        if !path.starts_with('/') {
            self.error(
                span,
                "a route path starts with `/`",
                "add a leading `/`",
                None,
            );
            return None;
        }
        let mut segs = Vec::new();
        let mut rest = path;
        while !rest.is_empty() {
            match rest.find('{') {
                None => {
                    if rest.contains('}') {
                        self.error(span, "unmatched `}` in the path", "no opening `{`", None);
                        return None;
                    }
                    segs.push(Seg::Lit(rest.to_string()));
                    break;
                }
                Some(i) => {
                    let (lit, tail) = rest.split_at(i);
                    if lit.contains('}') {
                        self.error(span, "unmatched `}` in the path", "no opening `{`", None);
                        return None;
                    }
                    if !lit.is_empty() {
                        segs.push(Seg::Lit(lit.to_string()));
                    }
                    let Some(end) = tail.find('}') else {
                        self.error(span, "unclosed `{` in the path", "add a `}`", None);
                        return None;
                    };
                    let name = &tail[1..end];
                    let valid = !name.is_empty()
                        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                        && !name.starts_with(|c: char| c.is_ascii_digit());
                    if !valid {
                        self.error(
                            span,
                            format!("`{{{name}}}` is not a valid path parameter name"),
                            "use letters, digits and `_`",
                            None,
                        );
                        return None;
                    }
                    segs.push(Seg::Param(name.to_string()));
                    rest = &tail[end + 1..];
                }
            }
        }
        Some(segs)
    }

    // ---- rpc ----

    fn lower_rpc(&mut self, r: &a::RpcBlock) {
        if !self.services.insert(r.name.name.clone()) {
            self.error(
                &r.name.span,
                format!("rpc service `{}` is declared twice", r.name.name),
                "duplicate service",
                None,
            );
            return;
        }
        let mut methods = Vec::new();
        let mut names = HashSet::new();
        for m in &r.methods {
            if !names.insert(m.name.name.clone()) {
                self.error(
                    &m.name.span,
                    format!(
                        "method `{}` is declared twice in `{}`",
                        m.name.name, r.name.name
                    ),
                    "duplicate method",
                    None,
                );
                continue;
            }
            if let Some(m) = self.lower_rpc_method(m) {
                methods.push(m);
            }
        }
        self.ir.rpc_services.push(RpcServiceDef {
            name: r.name.name.clone(),
            methods,
        });
    }

    fn lower_rpc_method(&mut self, m: &a::RpcMethod) -> Option<RpcMethodDef> {
        let anns = self.annotations(&m.annotations, Target::Operation, None);
        let ctx = pascal(&m.name.name);
        let request_name = format!("{ctx}Request");
        let unnamed = m.params.iter().filter(|p| p.name.is_none()).count();
        let mut client_streaming = false;

        let request = if m.params.is_empty() || unnamed == 0 {
            let fields = self.params_to_fields(&m.params, &request_name)?;
            self.add_record(&request_name, &m.name.span, fields)?
        } else if m.params.len() == 1 {
            let p = &m.params[0];
            client_streaming = p.stream;
            let ty = self.resolve_type(&p.ty, &request_name)?;
            if !self.require_record(&ty, &p.ty.span, "an rpc request") {
                return None;
            }
            ty
        } else {
            self.error(
                &m.name.span,
                format!(
                    "method `{}` mixes named and unnamed parameters",
                    m.name.name
                ),
                "use one unnamed request type, or only named parameters",
                None,
            );
            return None;
        };
        let result = self.lower_result(&m.result, &ctx, true);
        Some(RpcMethodDef {
            name: m.name.name.clone(),
            doc: m.doc.clone(),
            request,
            client_streaming,
            result: result?,
            deprecated: anns.deprecated,
        })
    }

    /// Named parameters as record fields.
    fn params_to_fields(&mut self, params: &[a::Param], owner: &str) -> Option<Vec<FieldDef>> {
        let mut fields = Vec::new();
        let mut names = HashSet::new();
        let mut ok = true;
        for p in params {
            let name = p.name.as_ref().unwrap();
            if !names.insert(name.name.clone()) {
                self.error(
                    &name.span,
                    format!("parameter `{}` is declared twice", name.name),
                    "duplicate",
                    None,
                );
                ok = false;
                continue;
            }
            match self.resolve_type(&p.ty, &format!("{owner}{}", pascal(&name.name))) {
                Some(ty) => fields.push(FieldDef {
                    name: name.name.clone(),
                    wire_name: name.name.clone(),
                    ty,
                    optional: p.optional,
                    doc: vec![],
                    deprecated: None,
                    rules: vec![],
                }),
                None => ok = false,
            }
        }
        ok.then_some(fields)
    }

    /// Register a generated record and return a reference to it.
    fn add_record(&mut self, name: &str, span: &Span, fields: Vec<FieldDef>) -> Option<TypeRef> {
        if !self.taken.insert(name.to_string()) {
            self.error(
                span,
                format!("the generated type name `{name}` is already used"),
                "a type with this name exists",
                Some("rename the method, or rename the type that uses this name".into()),
            );
            return None;
        }
        self.ir.types.push(TypeDef {
            name: name.to_string(),
            doc: vec![],
            params: vec![],
            inline: true,
            deprecated: None,
            body: TypeBody::Record { fields },
        });
        Some(TypeRef::Named {
            package: self.package.to_string(),
            name: name.to_string(),
            args: vec![],
        })
    }

    // ---- graphql ----

    fn lower_graphql(&mut self, g: &a::GraphqlBlock) {
        for f in &g.fields {
            let anns = self.annotations(&f.annotations, Target::Operation, None);
            let (kind, word) = match f.kind {
                a::GraphqlKind::Query => (GraphqlKind::Query, "query"),
                a::GraphqlKind::Mutation => (GraphqlKind::Mutation, "mutation"),
                a::GraphqlKind::Subscription => (GraphqlKind::Subscription, "subscription"),
            };
            if self
                .ir
                .graphql
                .iter()
                .any(|x| x.kind == kind && x.name == f.name.name)
            {
                self.error(
                    &f.name.span,
                    format!("{word} `{}` is declared twice", f.name.name),
                    "duplicate field",
                    None,
                );
                continue;
            }
            let ctx = pascal(&f.name.name);
            let mut args = Vec::new();
            let mut arg_names = HashSet::new();
            let mut ok = true;
            for p in &f.params {
                let Some(name) = &p.name else {
                    self.error(
                        &p.span,
                        "a GraphQL argument needs a name",
                        "write `name: Type`",
                        None,
                    );
                    ok = false;
                    continue;
                };
                if !arg_names.insert(name.name.clone()) {
                    self.error(
                        &name.span,
                        format!("argument `{}` is declared twice", name.name),
                        "duplicate",
                        None,
                    );
                    ok = false;
                    continue;
                }
                match self.resolve_type(&p.ty, &format!("{ctx}{}", pascal(&name.name))) {
                    Some(ty) => args.push(ParamDef {
                        name: name.name.clone(),
                        ty,
                        optional: p.optional,
                    }),
                    None => ok = false,
                }
            }
            let result = self.lower_result(&f.result, &ctx, false);
            if let (true, Some(result)) = (ok, result) {
                self.ir.graphql.push(GraphqlFieldDef {
                    kind,
                    name: f.name.name.clone(),
                    doc: f.doc.clone(),
                    args,
                    result,
                    deprecated: anns.deprecated,
                });
            }
        }
    }

    // ---- sockets ----

    fn lower_socket(&mut self, s: &a::SocketBlock) {
        if !self.sockets.insert(s.name.name.clone()) {
            self.error(
                &s.name.span,
                format!("socket `{}` is declared twice", s.name.name),
                "duplicate socket",
                None,
            );
            return;
        }
        let mut events = Vec::new();
        let mut names = HashSet::new();
        for e in &s.events {
            let anns = self.annotations(&e.annotations, Target::Operation, None);
            let name = e.name.text();
            if !names.insert(name.clone()) {
                self.error(
                    &e.name.span,
                    format!(
                        "event `{name}` is declared twice in socket `{}`",
                        s.name.name
                    ),
                    "duplicate event",
                    None,
                );
                continue;
            }
            let Some(payload) = self.resolve_type(&e.ty, &pascal(&name)) else {
                continue;
            };
            if !self.require_record(&payload, &e.ty.span, "a socket event payload") {
                continue;
            }
            events.push(SocketEventDef {
                name,
                doc: e.doc.clone(),
                side: match e.side {
                    a::Side::Server => Side::Server,
                    a::Side::Client => Side::Client,
                },
                payload,
                deprecated: anns.deprecated,
            });
        }
        self.ir.sockets.push(SocketDef {
            name: s.name.name.clone(),
            events,
        });
    }
}

enum Seg {
    Lit(String),
    Param(String),
}
