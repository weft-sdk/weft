//! The Go generator: types, the embedded contracts, and the typed `weft` object.
//!
//! The output is `gofmt`-clean by construction (fields and constants are aligned the way gofmt aligns
//! them, imports are sorted), so formatting the file never changes it and `weft gen --check` stays stable.

use std::collections::HashSet;
use std::fmt::Write;

use weft_ir::{ErrorDef, EventDef, FieldDef, Ir, Primitive, RouteDef, TypeBody, TypeDef, TypeRef};

use crate::{CodegenError, Int64, Options};

/// Words that Go style writes in capitals.
const INITIALISMS: [&str; 12] = [
    "id", "url", "uri", "uuid", "http", "https", "api", "json", "html", "sql", "ip", "tcp",
];

/// Generate `weft.gen.go` for the given packages.
pub fn go(irs: &[Ir], options: &Options) -> Result<String, Vec<CodegenError>> {
    Gen::new(irs, options).run()
}

struct Gen<'a> {
    irs: &'a [Ir],
    options: &'a Options,
    errors: Vec<CodegenError>,
    uses_time: bool,
}

/// A route's body, query and headers types, and its path parameters as (field, type).
struct RouteParts {
    body: Option<String>,
    query: Option<String>,
    headers: Option<String>,
    params: Vec<(String, String)>,
}

#[derive(Default)]
struct Plan {
    http: bool,
    kafka: bool,
    skipped: Vec<String>,
}

fn words(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut prev_lower = false;
    for c in s.chars() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        current.push(c);
        prev_lower = c.is_lowercase() || c.is_numeric();
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// `customerId` -> `CustomerID`, `order.placed` -> `OrderPlaced`.
fn pascal(s: &str) -> String {
    words(s)
        .iter()
        .map(|w| {
            let lower = w.to_lowercase();
            if INITIALISMS.contains(&lower.as_str()) {
                lower.to_uppercase()
            } else {
                let mut cs = w.chars();
                match cs.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect()
}

/// A Go string literal that is safe for any text: a raw string, with backticks spliced in.
fn raw_string(text: &str) -> String {
    format!("`{}`", text.replace('`', "` + \"`\" + `"))
}

/// Lines of columns, aligned the way gofmt aligns a block: every column but the last is padded to the
/// widest cell in the block, plus one space. A block ends at a comment or blank line.
fn aligned(rows: &[Vec<String>]) -> Vec<String> {
    if rows.is_empty() {
        return Vec::new();
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; columns];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i + 1 < row.len() {
                widths[i] = widths[i].max(cell.chars().count());
            }
        }
    }
    rows.iter()
        .map(|row| {
            let mut line = String::new();
            for (i, cell) in row.iter().enumerate() {
                line.push_str(cell);
                if i + 1 < row.len() {
                    let pad = widths[i] - cell.chars().count() + 1;
                    line.push_str(&" ".repeat(pad));
                }
            }
            line
        })
        .collect()
}

/// One line of a `//` comment: an empty line is `//` with nothing after it, as gofmt writes it.
fn comment(line: &str) -> String {
    if line.is_empty() {
        "//".to_string()
    } else {
        format!("// {line}")
    }
}

/// A field or constant, with the doc lines that come before it.
struct Item {
    doc: Vec<String>,
    cells: Vec<String>,
}

/// Items, one per line at `depth` tabs. Items with a doc comment start a new aligned block.
fn block(items: &[Item], depth: usize) -> String {
    let pad = "\t".repeat(depth);
    let mut out = String::new();
    let mut run: Vec<Vec<String>> = Vec::new();
    let flush = |run: &mut Vec<Vec<String>>, out: &mut String| {
        for line in aligned(run) {
            let _ = writeln!(out, "{pad}{line}");
        }
        run.clear();
    };
    for item in items {
        if !item.doc.is_empty() {
            flush(&mut run, &mut out);
            for d in &item.doc {
                let _ = writeln!(out, "{pad}{}", comment(d));
            }
        }
        run.push(item.cells.clone());
        if !item.doc.is_empty() {
            // The doc comment above separates this item from the next block too, which is fine:
            // the run continues until the next documented item.
        }
    }
    flush(&mut run, &mut out);
    out
}

fn doc_lines(doc: &[String], deprecated: Option<&str>) -> Vec<String> {
    let mut lines: Vec<String> = doc.iter().map(|l| l.trim_end().to_string()).collect();
    if let Some(d) = deprecated {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(if d.is_empty() {
            "Deprecated.".to_string()
        } else {
            format!("Deprecated: {d}")
        });
    }
    lines
}

impl<'a> Gen<'a> {
    fn new(irs: &'a [Ir], options: &'a Options) -> Self {
        Gen {
            irs,
            options,
            errors: Vec::new(),
            uses_time: false,
        }
    }

    fn fail(&mut self, message: String, help: &str) {
        self.errors.push(CodegenError {
            message,
            help: Some(help.to_string()),
        });
    }

    fn plan(&self) -> Option<Plan> {
        let mut plan = Plan::default();
        for t in &self.options.transports {
            let lib = t.library.as_deref().map(|l| l.to_lowercase());
            match (t.name.as_str(), lib.as_deref()) {
                ("http", None | Some("nethttp" | "net/http" | "stdlib")) => plan.http = true,
                ("kafka", None | Some("franz" | "franz-go")) => plan.kafka = true,
                (name, Some(l)) => plan.skipped.push(format!("{name} ({l})")),
                (name, None) => plan.skipped.push(name.to_string()),
            }
        }
        (plan.http || plan.kafka).then_some(plan)
    }

    /// The Go name of a type declared in a package.
    fn type_name(pkg: &str, name: &str) -> String {
        format!("{}{}", pascal(pkg), pascal(name))
    }

    fn run(mut self) -> Result<String, Vec<CodegenError>> {
        let plan = self.plan();
        let package = self
            .options
            .package
            .clone()
            .unwrap_or_else(|| "weftgen".to_string());

        // Names must be unique and be Go identifiers.
        let mut seen: HashSet<String> = HashSet::new();
        for ir in self.irs {
            for t in &ir.types {
                let n = Self::type_name(&ir.package, &t.name);
                if !seen.insert(n.clone()) {
                    self.fail(
                        format!("two types are called `{n}` in Go"),
                        "rename one of them, or one of the packages",
                    );
                }
            }
            for e in &ir.errors {
                let n = Self::type_name(&ir.package, &e.name);
                if !seen.insert(n.clone()) {
                    self.fail(
                        format!(
                            "the error `{}` and a type are both called `{n}` in Go",
                            e.name
                        ),
                        "rename one of them",
                    );
                }
            }
        }
        let mut events: Vec<(&EventDef, &Ir)> = Vec::new();
        for ir in self.irs {
            for e in &ir.events {
                if let Some((_, other)) = events.iter().find(|(x, _)| x.name == e.name) {
                    let (a, b) = (other.package.clone(), ir.package.clone());
                    self.fail(
                        format!("the event `{}` is declared in both `{a}` and `{b}`", e.name),
                        "the typed `weft` object names events by their name alone: rename one",
                    );
                }
                events.push((e, ir));
            }
        }
        if !self.errors.is_empty() {
            return Err(self.errors);
        }

        let mut body = String::new();
        body.push_str("// UUID is a uuid. It is its own type so that an arbitrary string is not taken for one.\ntype UUID string\n");
        for ir in self.irs {
            for t in &ir.types {
                body.push('\n');
                self.type_def(&mut body, ir, t);
            }
            for e in &ir.errors {
                body.push('\n');
                self.error_def(&mut body, ir, e);
            }
        }
        self.contracts(&mut body);
        let facade = plan.as_ref().map(|p| {
            let mut f = String::new();
            self.facade(&mut f, p, &events);
            f
        });
        if let Some(f) = &facade {
            body.push_str(f);
        }
        if !self.errors.is_empty() {
            return Err(self.errors);
        }

        let mut out = String::new();
        let _ = writeln!(
            out,
            "// Code generated by {}. DO NOT EDIT: change the contracts and run `weft gen`.",
            self.options.generator
        );
        let packages: Vec<&str> = self.irs.iter().map(|i| i.package.as_str()).collect();
        let _ = writeln!(out, "// Contracts: {}.\n", packages.join(", "));
        let _ = writeln!(out, "package {package}\n");
        out.push_str("import (\n");
        let mut std_imports = vec!["encoding/json", "sync"];
        if facade.is_some() {
            std_imports.extend(["context", "log/slog"]);
        }
        if self.uses_time || plan.as_ref().is_some_and(|p| p.http) {
            std_imports.push("time");
        }
        std_imports.sort_unstable();
        for i in std_imports {
            let _ = writeln!(out, "\t\"{i}\"");
        }
        out.push('\n');
        let mut third = vec!["weft \"github.com/weft-sdk/weft/go\"".to_string()];
        if let Some(p) = &plan {
            if p.http {
                third.push("\"github.com/weft-sdk/weft/go/http/nethttp\"".into());
            }
            if p.kafka {
                third.push("\"github.com/weft-sdk/weft/go/kafka/franz\"".into());
            }
        }
        // gofmt sorts imports by path.
        third.sort_by_key(|i| i.split('"').nth(1).unwrap_or("").to_string());
        for i in third {
            let _ = writeln!(out, "\t{i}");
        }
        out.push_str(")\n\n");
        out.push_str(&body);
        Ok(out)
    }

    // ---- types ----

    fn go_ty(&mut self, ty: &TypeRef) -> String {
        match ty {
            TypeRef::Primitive { name } => match name {
                Primitive::String | Primitive::Decimal => "string".into(),
                Primitive::Bool => "bool".into(),
                Primitive::Int | Primitive::Int64 => match self.options.int64 {
                    Int64::Number | Int64::Bigint => "int64".into(),
                },
                Primitive::Int32 => "int32".into(),
                Primitive::Float | Primitive::Float64 => "float64".into(),
                Primitive::Float32 => "float32".into(),
                Primitive::Uuid => "UUID".into(),
                Primitive::Timestamp => {
                    self.uses_time = true;
                    "time.Time".into()
                }
                Primitive::Bytes => "[]byte".into(),
            },
            TypeRef::Named {
                package,
                name,
                args,
            } => {
                let mut s = Self::type_name(package, name);
                if !args.is_empty() {
                    let a: Vec<String> = args.iter().map(|a| self.go_ty(a)).collect();
                    let _ = write!(s, "[{}]", a.join(", "));
                }
                s
            }
            TypeRef::Param { name } => name.clone(),
            TypeRef::List { item } => format!("[]{}", self.go_ty(item)),
            TypeRef::Map { key, value } => {
                let k = self.go_ty(key);
                let v = self.go_ty(value);
                format!("map[{k}]{v}")
            }
        }
    }

    fn fields(&mut self, fields: &[FieldDef]) -> Vec<Item> {
        fields
            .iter()
            .map(|f| {
                let ty = self.go_ty(&f.ty);
                let indirect = !matches!(f.ty, TypeRef::List { .. } | TypeRef::Map { .. });
                let (ty, tag) = if f.optional {
                    (
                        if indirect { format!("*{ty}") } else { ty },
                        format!("`json:\"{},omitempty\"`", f.wire_name),
                    )
                } else {
                    (ty, format!("`json:\"{}\"`", f.wire_name))
                };
                Item {
                    doc: doc_lines(&f.doc, f.deprecated.as_deref()),
                    cells: vec![pascal(&f.name), ty, tag],
                }
            })
            .collect()
    }

    fn type_def(&mut self, out: &mut String, ir: &Ir, t: &TypeDef) {
        let name = Self::type_name(&ir.package, &t.name);
        for l in doc_lines(&t.doc, t.deprecated.as_deref()) {
            let _ = writeln!(out, "{}", comment(&l));
        }
        match &t.body {
            TypeBody::Enum { variants } => {
                let _ = writeln!(out, "type {name} string\n");
                out.push_str("const (\n");
                let items: Vec<Item> = variants
                    .iter()
                    .map(|v| Item {
                        doc: doc_lines(&v.doc, v.deprecated.as_deref()),
                        cells: vec![
                            format!("{name}{}", pascal(&v.name)),
                            name.clone(),
                            format!("= \"{}\"", v.name),
                        ],
                    })
                    .collect();
                out.push_str(&block(&items, 1));
                out.push_str(")\n");
            }
            TypeBody::Record { fields } => {
                let params = if t.params.is_empty() {
                    String::new()
                } else {
                    format!(
                        "[{}]",
                        t.params
                            .iter()
                            .map(|p| format!("{p} any"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                if fields.is_empty() {
                    let _ = writeln!(out, "type {name}{params} struct{{}}");
                } else {
                    let _ = writeln!(out, "type {name}{params} struct {{");
                    let items = self.fields(fields);
                    out.push_str(&block(&items, 1));
                    out.push_str("}\n");
                }
            }
        }
    }

    fn error_def(&mut self, out: &mut String, ir: &Ir, e: &ErrorDef) {
        let name = Self::type_name(&ir.package, &e.name);
        let mut doc = e.doc.clone();
        doc.push(format!(
            "{name} holds the fields of the error {}.{}, HTTP status {}. A handler returns it with `New{name}`; a caller reads it with `As{name}`.",
            ir.package, e.name, e.status
        ));
        for l in doc_lines(&doc, None) {
            let _ = writeln!(out, "{}", comment(&l));
        }
        if e.fields.is_empty() {
            let _ = writeln!(out, "type {name} struct{{}}");
        } else {
            let _ = writeln!(out, "type {name} struct {{");
            let items = self.fields(&e.fields);
            out.push_str(&block(&items, 1));
            out.push_str("}\n");
        }
        let _ = writeln!(
            out,
            "\n// New{name} makes the error a handler returns for {}.{}.\nfunc New{name}(fields {name}, message string) *weft.DeclaredError {{\n\treturn Contracts().NewDeclaredError(\"{}\", \"{}\", fields, message)\n}}",
            ir.package, e.name, ir.package, e.name
        );
        let _ = writeln!(
            out,
            "\n// As{name} reads err as {}.{}, if it is that error.\nfunc As{name}(err error) ({name}, bool) {{\n\treturn weft.DeclaredFields[{name}](Contracts(), err, \"{}\", \"{}\")\n}}",
            ir.package, e.name, ir.package, e.name
        );
    }

    // ---- the contracts ----

    fn contracts(&self, out: &mut String) {
        let docs: Vec<String> = self
            .irs
            .iter()
            .map(|ir| serde_json::to_string(ir).expect("the IR always serialises"))
            .collect();
        let json = format!("[{}]", docs.join(","));
        let _ = writeln!(
            out,
            "\n// contractsJSON is the checked contracts of every package, which the runtime reads to validate and encode.\nconst contractsJSON = {}\n",
            raw_string(&json)
        );
        out.push_str(
            "var (\n\tcontractsOnce sync.Once\n\tcontractsVal  *weft.Contracts\n)\n\n// Contracts returns the contracts, parsed once.\nfunc Contracts() *weft.Contracts {\n\tcontractsOnce.Do(func() {\n\t\tvar irs []weft.Ir\n\t\tif err := json.Unmarshal([]byte(contractsJSON), &irs); err != nil {\n\t\t\tpanic(\"the embedded contracts are not valid: \" + err.Error())\n\t\t}\n\t\tcontractsVal = weft.NewContracts(irs...)\n\t})\n\treturn contractsVal\n}\n",
        );
    }

    /// A Go expression for a type reference.
    fn ref_expr(&self, ty: &TypeRef) -> String {
        match ty {
            TypeRef::Primitive { name } => {
                let n = serde_json::to_value(name).expect("a primitive serialises");
                format!("weft.Primitive(\"{}\")", n.as_str().unwrap_or(""))
            }
            TypeRef::Named {
                package,
                name,
                args,
            } => {
                let mut s = format!("weft.Named(\"{package}\", \"{name}\"");
                for a in args {
                    let _ = write!(s, ", {}", self.ref_expr(a));
                }
                s.push(')');
                s
            }
            TypeRef::Param { name } => format!("weft.TypeRef{{Kind: \"param\", Name: \"{name}\"}}"),
            TypeRef::List { item } => format!("weft.List({})", self.ref_expr(item)),
            TypeRef::Map { key, value } => {
                format!("weft.Map({}, {})", self.ref_expr(key), self.ref_expr(value))
            }
        }
    }

    // ---- the typed weft object ----

    fn facade(&mut self, out: &mut String, plan: &Plan, events: &[(&EventDef, &Ir)]) {
        out.push_str("\n// ---- The typed `weft` object ----\n");
        if !plan.skipped.is_empty() {
            let _ = writeln!(
                out,
                "// Not available in the Go package yet, so not in `Weft`: {}.",
                plan.skipped.join(", ")
            );
        }

        // Options, Weft, New.
        out.push_str(
            "\n// Options are what New needs. Everything is optional.\ntype Options struct {\n",
        );
        let mut opt_items = vec![
            Item {
                doc: vec![
                    "Config is weft.yaml. Default: the one found from the working folder.".into(),
                ],
                cells: vec!["Config".into(), "*weft.Config".into()],
            },
            Item {
                doc: vec!["Logger is where Weft logs. Default: slog's default.".into()],
                cells: vec!["Logger".into(), "*slog.Logger".into()],
            },
        ];
        if plan.http {
            opt_items.push(Item {
                doc: vec!["HTTPAdapter replaces net/http, for tests.".into()],
                cells: vec!["HTTPAdapter".into(), "weft.HTTPAdapter".into()],
            });
        }
        if plan.kafka {
            opt_items.push(Item {
                doc: vec!["KafkaAdapter replaces franz-go, for tests.".into()],
                cells: vec!["KafkaAdapter".into(), "weft.MessagingAdapter".into()],
            });
        }
        out.push_str(&block(&opt_items, 1));
        out.push_str("}\n");

        out.push_str("\n// Weft is weft.*: the lifecycle, and a typed object for each transport.\ntype Weft struct {\n");
        let mut w_items = vec![Item {
            doc: vec!["Core is the untyped runtime underneath.".into()],
            cells: vec!["Core".into(), "*weft.Weft".into()],
        }];
        if plan.http {
            w_items.push(Item {
                doc: vec!["HTTP is weft.http.".into()],
                cells: vec!["HTTP".into(), "*HTTP".into()],
            });
        }
        if plan.kafka {
            w_items.push(Item {
                doc: vec!["Kafka is weft.kafka.".into()],
                cells: vec!["Kafka".into(), "*Kafka".into()],
            });
        }
        out.push_str(&block(&w_items, 1));
        out.push_str("}\n");

        out.push_str("\n// New builds Weft from weft.yaml (or o.Config) and the embedded contracts. Register handlers, then Start.\nfunc New(o Options) (*Weft, error) {\n\tconfig := o.Config\n\tif config == nil {\n\t\tvar err error\n\t\tif config, err = weft.LoadConfig(\"\"); err != nil {\n\t\t\treturn nil, err\n\t\t}\n\t}\n\tcore := weft.New(weft.Options{Contracts: Contracts(), Config: config})\n\tw := &Weft{Core: core}\n");
        if plan.http {
            out.push_str("\tadapter := o.HTTPAdapter\n\tif adapter == nil {\n\t\tadapter = nethttp.New(nethttp.Options{})\n\t}\n\th, err := weft.NewHTTP(core, weft.HTTPOptions{Adapter: adapter, Logger: o.Logger})\n\tif err != nil {\n\t\treturn nil, err\n\t}\n\tw.HTTP = &HTTP{h: h}\n");
        }
        if plan.kafka {
            out.push_str("\tkafka := o.KafkaAdapter\n\tif kafka == nil {\n\t\tkafka = franz.New(franz.Options{})\n\t}\n\tm, err := weft.NewMessaging(core, weft.MessagingOptions{Adapter: kafka, Name: \"kafka\", Logger: o.Logger})\n\tif err != nil {\n\t\treturn nil, err\n\t}\n\tw.Kafka = &Kafka{m: m}\n");
        }
        out.push_str("\treturn w, nil\n}\n");
        out.push_str("\n// Start connects every transport. Register handlers first.\nfunc (w *Weft) Start(ctx context.Context) error { return w.Core.Start(ctx) }\n");
        out.push_str("\n// Stop stops taking new work, finishes what is running, and closes connections.\nfunc (w *Weft) Stop(ctx context.Context) weft.StopResult { return w.Core.Stop(ctx) }\n");
        out.push_str("\n// Use adds middleware for every transport.\nfunc (w *Weft) Use(m weft.Middleware) { w.Core.Use(m) }\n");

        if plan.kafka {
            out.push_str("\n// Kafka is weft.kafka, typed from the contracts.\ntype Kafka struct{ m *weft.Messaging }\n");
            out.push_str("\n// Messaging is the untyped messaging underneath.\nfunc (k *Kafka) Messaging() *weft.Messaging { return k.m }\n");
            out.push_str("\n// Raw is the franz-go client. Weft's tracing and middleware do not apply to calls made through it.\nfunc (k *Kafka) Raw() any { return k.m.Raw() }\n");
            out.push_str("\n// Use adds middleware for Kafka only.\nfunc (k *Kafka) Use(mw weft.MessageMiddleware) { k.m.Use(mw) }\n");
            for (e, ir) in events {
                let method = pascal(&e.name);
                let ty = self.go_ty(&e.payload);
                // The event name is in the payload type's package-qualified form already.
                let _ = ir;
                let _ = writeln!(
                    out,
                    "\n// Publish{method} sends {name}.\nfunc (k *Kafka) Publish{method}(ctx context.Context, payload {ty}, o weft.PublishOptions) error {{\n\to.Package = \"{pkg}\"\n\treturn weft.PublishTyped(ctx, k.m, \"{name}\", payload, o)\n}}",
                    name = e.name,
                    pkg = ir.package
                );
                let _ = writeln!(
                    out,
                    "\n// On{method} receives {name}.\nfunc (k *Kafka) On{method}(handler func(ctx context.Context, payload {ty}, mc *weft.MessageContext) error, o weft.SubscribeOptions) error {{\n\to.Package = \"{pkg}\"\n\treturn weft.OnTyped(k.m, \"{name}\", handler, o)\n}}",
                    name = e.name,
                    pkg = ir.package
                );
            }
        }

        if plan.http {
            self.http(out);
        }
    }

    fn http(&mut self, out: &mut String) {
        out.push_str("\n// HTTP is weft.http, typed from the contracts: handlers for the routes this service serves, and a\n// client for each package.\ntype HTTP struct{ h *weft.HTTP }\n");
        out.push_str("\n// HTTP is the untyped HTTP underneath.\nfunc (h *HTTP) HTTP() *weft.HTTP { return h.h }\n");
        out.push_str("\n// Raw is the *http.ServeMux. Weft's tracing and middleware do not apply to calls made through it.\nfunc (h *HTTP) Raw() any { return h.h.Raw() }\n");
        out.push_str("\n// Use adds middleware for HTTP only.\nfunc (h *HTTP) Use(mw weft.HTTPMiddleware) { h.h.Use(mw) }\n");
        out.push_str("\n// NotFound is the error to return for \"nothing here\".\nfunc (h *HTTP) NotFound(message string) error { return h.h.NotFound(message) }\n");

        let service = self.options.service.clone();
        let irs = self.irs;
        for ir in irs {
            let client = format!("{}Client", pascal(&ir.package));
            let _ = writeln!(
                out,
                "\n// {client} calls the routes of {pkg}.\ntype {client} struct{{ c *weft.ServiceClient }}\n\n// {pascal} returns a client for the service {pkg}, whose URL is under services: in weft.yaml.\nfunc (h *HTTP) {pascal}() *{client} {{ return &{client}{{c: h.h.To(\"{pkg}\")}} }}",
                pkg = ir.package,
                pascal = pascal(&ir.package)
            );
            for r in &ir.routes {
                self.route_client(out, ir, r, &client);
                if service.as_deref() == Some(ir.package.as_str()) {
                    self.route_server(out, ir, r);
                }
            }
        }
    }

    /// The pieces a route has, as Go types.
    fn route_parts(&mut self, r: &RouteDef) -> RouteParts {
        RouteParts {
            body: r.body.as_ref().map(|t| self.go_ty(t)),
            query: r.query.as_ref().map(|t| self.go_ty(t)),
            headers: r.headers.as_ref().map(|t| self.go_ty(t)),
            params: r
                .path_params
                .iter()
                .map(|p| (pascal(&p.name), self.go_ty(&p.ty)))
                .collect(),
        }
    }

    fn route_structs(&mut self, out: &mut String, name: &str, r: &RouteDef, server: bool) {
        let RouteParts {
            body,
            query,
            headers,
            params,
        } = self.route_parts(r);
        let mut items: Vec<Item> = Vec::new();
        if let Some(b) = &body {
            items.push(Item {
                doc: vec![],
                cells: vec!["Body".into(), b.clone()],
            });
        }
        if !params.is_empty() {
            items.push(Item {
                doc: vec![],
                cells: vec!["Params".into(), format!("{name}Params")],
            });
        }
        if let Some(q) = &query {
            items.push(Item {
                doc: vec![],
                cells: vec!["Query".into(), q.clone()],
            });
        }
        if let Some(h) = &headers {
            items.push(Item {
                doc: vec![],
                cells: vec!["Headers".into(), h.clone()],
            });
        }
        if server {
            items.push(Item {
                doc: vec![],
                cells: vec!["Ctx".into(), "*weft.HTTPContext".into()],
            });
        } else {
            items.push(Item {
                doc: vec!["Timeout overrides the 10 s call timeout.".into()],
                cells: vec!["Timeout".into(), "time.Duration".into()],
            });
            items.push(Item {
                doc: vec![
                    "IdempotencyKey is sent as Idempotency-Key, which makes a POST safe to retry."
                        .into(),
                ],
                cells: vec!["IdempotencyKey".into(), "string".into()],
            });
        }
        if !params.is_empty() {
            let _ = writeln!(
                out,
                "\n// {name}Params are the path parameters of {name}.\ntype {name}Params struct {{"
            );
            let p_items: Vec<Item> = params
                .iter()
                .map(|(n, t)| Item {
                    doc: vec![],
                    cells: vec![n.clone(), t.clone()],
                })
                .collect();
            out.push_str(&block(&p_items, 1));
            out.push_str("}\n");
        }
        let suffix = if server { "Input" } else { "Request" };
        let doc = if server {
            format!("{name}Input is what the handler of {name} is given, decoded and checked against the route.")
        } else {
            format!("{name}Request is what to send to call {name}.")
        };
        let _ = writeln!(out, "\n// {doc}");
        let _ = writeln!(out, "type {name}{suffix} struct {{");
        out.push_str(&block(&items, 1));
        out.push_str("}\n");
    }

    fn route_server(&mut self, out: &mut String, ir: &Ir, r: &RouteDef) {
        let name = pascal(&r.name);
        self.route_structs(out, &name, r, true);
        let RouteParts {
            body,
            query,
            headers,
            params,
        } = self.route_parts(r);
        let result = self.go_ty(&r.result.value);
        let ret = if r.result.optional {
            format!("*{result}")
        } else {
            result.clone()
        };
        let method = format!("{:?}", r.method).to_uppercase();
        let path = format!("{}{}", r.base.as_deref().unwrap_or(""), r.path);
        let _ = writeln!(
            out,
            "\n// Handle{name} serves {method} {path}.{opt}\nfunc (h *HTTP) Handle{name}(handler func(ctx context.Context, in {name}Input) ({ret}, error)) error {{\n\treturn h.h.HandleIn(\"{pkg}\", \"{method}\", \"{path}\", func(ctx context.Context, raw *weft.HTTPInput) (any, error) {{\n\t\tin := {name}Input{{Ctx: raw.Ctx}}",
            opt = if r.result.optional { " Returning nil answers 404." } else { "" },
            pkg = ir.package
        );
        if body.is_some() {
            let _ = writeln!(out, "\t\tif err := Contracts().FromCode({}, raw.Body, &in.Body); err != nil {{\n\t\t\treturn nil, err\n\t\t}}", self.ref_expr(r.body.as_ref().expect("checked")));
        }
        for (p, (field, _)) in r.path_params.iter().zip(&params) {
            let _ = writeln!(out, "\t\tif err := Contracts().FromCode({}, raw.Params[\"{}\"], &in.Params.{field}); err != nil {{\n\t\t\treturn nil, err\n\t\t}}", self.ref_expr(&p.ty), p.name);
        }
        if query.is_some() {
            let _ = writeln!(out, "\t\tif err := Contracts().FromCode({}, raw.Query, &in.Query); err != nil {{\n\t\t\treturn nil, err\n\t\t}}", self.ref_expr(r.query.as_ref().expect("checked")));
        }
        if headers.is_some() {
            let _ = writeln!(out, "\t\tif err := Contracts().FromCode({}, raw.Headers, &in.Headers); err != nil {{\n\t\t\treturn nil, err\n\t\t}}", self.ref_expr(r.headers.as_ref().expect("checked")));
        }
        let _ = writeln!(
            out,
            "\t\tout, err := handler(ctx, in)\n\t\tif err != nil {{\n\t\t\treturn nil, err\n\t\t}}"
        );
        if r.result.optional {
            out.push_str("\t\tif out == nil {\n\t\t\treturn nil, nil\n\t\t}\n");
        }
        let _ = writeln!(
            out,
            "\t\treturn Contracts().ToCode({}, out)\n\t}})\n}}",
            self.ref_expr(&r.result.value)
        );
    }

    fn route_client(&mut self, out: &mut String, ir: &Ir, r: &RouteDef, client: &str) {
        let name = format!("{}{}", pascal(&ir.package), pascal(&r.name));
        self.route_structs(out, &name, r, false);
        let RouteParts {
            body,
            query,
            headers,
            params,
        } = self.route_parts(r);
        let result = self.go_ty(&r.result.value);
        let ret = if r.result.optional {
            format!("*{result}")
        } else {
            result.clone()
        };
        let method = format!("{:?}", r.method).to_uppercase();
        let path = format!("{}{}", r.base.as_deref().unwrap_or(""), r.path);
        let _ = writeln!(
            out,
            "\n// {pname} calls {method} {path} of {pkg}.{opt}\nfunc (c *{client}) {pname}(ctx context.Context, in {name}Request) ({ret}, error) {{\n\tvar o weft.CallOptions\n\to.Timeout, o.IdempotencyKey, o.Package = in.Timeout, in.IdempotencyKey, \"{pkg}\"\n\tvar err error",
            pname = pascal(&r.name),
            pkg = ir.package,
            opt = if r.result.optional { " A missing result is nil." } else { "" }
        );
        if body.is_some() {
            let _ = writeln!(out, "\tif o.Body, err = Contracts().ToCode({}, in.Body); err != nil {{\n\t\treturn {zero}, err\n\t}}", self.ref_expr(r.body.as_ref().expect("checked")), zero = zero(&ret));
        }
        if !params.is_empty() {
            out.push_str("\to.Params = map[string]any{}\n");
            for (p, (field, _)) in r.path_params.iter().zip(&params) {
                let _ = writeln!(out, "\tif o.Params[\"{}\"], err = Contracts().ToCode({}, in.Params.{field}); err != nil {{\n\t\treturn {zero}, err\n\t}}", p.name, self.ref_expr(&p.ty), zero = zero(&ret));
            }
        }
        if query.is_some() {
            let _ = writeln!(out, "\tq, err := Contracts().ToCode({}, in.Query)\n\tif err != nil {{\n\t\treturn {zero}, err\n\t}}\n\to.Query, _ = q.(map[string]any)", self.ref_expr(r.query.as_ref().expect("checked")), zero = zero(&ret));
        }
        if headers.is_some() {
            let _ = writeln!(out, "\thd, err := Contracts().ToCode({}, in.Headers)\n\tif err != nil {{\n\t\treturn {zero}, err\n\t}}\n\to.Headers, _ = hd.(map[string]any)", self.ref_expr(r.headers.as_ref().expect("checked")), zero = zero(&ret));
        }
        let _ = writeln!(
            out,
            "\tcode, err := c.c.Call(ctx, \"{method}\", \"{path}\", o)\n\tif err != nil {{\n\t\treturn {zero}, err\n\t}}",
            zero = zero(&ret)
        );
        if r.result.optional {
            out.push_str("\tif code == nil {\n\t\treturn nil, nil\n\t}\n");
            let _ = writeln!(out, "\tvar out {result}\n\tif err := Contracts().FromCode({}, code, &out); err != nil {{\n\t\treturn nil, err\n\t}}\n\treturn &out, nil\n}}", self.ref_expr(&r.result.value));
        } else {
            let _ = writeln!(out, "\tvar out {result}\n\terr = Contracts().FromCode({}, code, &out)\n\treturn out, err\n}}", self.ref_expr(&r.result.value));
        }
    }
}

/// The zero value of a returned type, written as Go.
fn zero(ty: &str) -> String {
    if ty.starts_with('*') || ty.starts_with("[]") || ty.starts_with("map[") {
        "nil".to_string()
    } else if ty == "string" || ty == "UUID" {
        "\"\"".to_string()
    } else if ty == "bool" {
        "false".to_string()
    } else if ty.starts_with("int") || ty.starts_with("float") {
        "0".to_string()
    } else {
        format!("{ty}{{}}")
    }
}
