use weft_syntax::ast::*;
use weft_syntax::parse;

const ORDERS: &str = include_str!("../../../../spec/examples/orders.weft");

fn ok(src: &str) -> File {
    match parse("test.weft", src) {
        Ok(f) => f,
        Err(errors) => panic!("{}", render(&errors)),
    }
}

fn render(errors: &[weft_syntax::SyntaxError]) -> String {
    use miette::{GraphicalReportHandler, GraphicalTheme};
    let handler =
        GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor()).with_width(80);
    errors
        .iter()
        .map(|e| {
            let mut out = String::new();
            handler.render_report(&mut out, e).unwrap();
            out
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn err(src: &str) -> String {
    render(&parse("test.weft", src).expect_err("expected a syntax error"))
}

#[test]
fn parses_the_complete_example() {
    let f = ok(ORDERS);
    assert_eq!(f.package.name.name, "orders");
    assert_eq!(f.package.annotations[0].name.name, "version");
    assert_eq!(
        f.package.annotations[0].args,
        vec![AnnotationArg::Str("1.0.0".into())]
    );
    let kinds: Vec<&str> = f
        .decls
        .iter()
        .map(|d| match &d.kind {
            DeclKind::Enum(_) => "enum",
            DeclKind::Type(_) => "type",
            DeclKind::Event(_) => "event",
            DeclKind::Error(_) => "error",
            DeclKind::Http(_) => "http",
            DeclKind::Rpc(_) => "rpc",
            DeclKind::Graphql(_) => "graphql",
            DeclKind::Socket(_) => "socket",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "enum", "type", "type", "type", "type", "event", "event", "error", "http", "rpc",
            "graphql", "socket"
        ]
    );
    assert!(!f.comments.is_empty());
}

#[test]
fn doc_comments_attach_to_declarations() {
    let f = ok(ORDERS);
    let order = f
        .decls
        .iter()
        .find(|d| matches!(&d.kind, DeclKind::Type(t) if t.name.name == "Order"))
        .unwrap();
    assert_eq!(order.doc, ["An order placed by a customer."]);
}

#[test]
fn optional_fields_lists_and_paths() {
    let f = ok("package p\nimport orders\ntype T { note?: string items: orders.Item[][] }");
    assert_eq!(f.imports[0].package.name, "orders");
    let DeclKind::Type(t) = &f.decls[0].kind else {
        panic!()
    };
    assert!(t.fields[0].optional);
    let TypeKind::List(inner) = &t.fields[1].ty.kind else {
        panic!()
    };
    assert!(matches!(inner.kind, TypeKind::List(_)));
}

#[test]
fn generics_and_nested_arguments() {
    let f = ok("package p\ntype Page<T> { items: T[] next?: string }\ntype X { a: Page<Page<int>> m: map<string, int> }");
    let DeclKind::Type(p) = &f.decls[0].kind else {
        panic!()
    };
    assert_eq!(p.params[0].name, "T");
    let DeclKind::Type(x) = &f.decls[1].kind else {
        panic!()
    };
    let TypeKind::Named { args, .. } = &x.fields[0].ty.kind else {
        panic!()
    };
    assert_eq!(args.len(), 1);
}

#[test]
fn a_field_may_be_named_like_a_keyword() {
    let f = ok("package p\ntype Result<T, E> { value?: T error?: E }\nevent a.b: { type: string, query: int }");
    let DeclKind::Type(t) = &f.decls[0].kind else {
        panic!()
    };
    assert_eq!(t.fields[1].name.name, "error");
}

#[test]
fn fields_may_be_separated_by_commas_or_nothing() {
    let a = ok("package p\ntype T { a: int, b: int, }");
    let b = ok("package p\ntype T { a: int b: int }");
    let c = ok("package p\ntype T {\n  a: int\n  b: int\n}");
    for f in [a, b, c] {
        let DeclKind::Type(t) = &f.decls[0].kind else {
            panic!()
        };
        assert_eq!(t.fields.len(), 2);
    }
}

#[test]
fn annotations_with_every_argument_kind() {
    let f = ok(r#"package p
type T {
  a: int @min(-1) @max(100)
  b: string @length(1, 200) @pattern("^[a-z]+$") @format(email)
  c: float @min(0.5) @deprecated("use d") @wire("c_old")
}"#);
    let DeclKind::Type(t) = &f.decls[0].kind else {
        panic!()
    };
    assert_eq!(
        t.fields[0].annotations[0].args,
        [AnnotationArg::Num("-1".into())]
    );
    assert_eq!(
        t.fields[1].annotations[2].args,
        [AnnotationArg::Ident("email".into())]
    );
    assert_eq!(
        t.fields[2].annotations[0].args,
        [AnnotationArg::Num("0.5".into())]
    );
}

#[test]
fn event_options_and_inline_payloads() {
    let f = ok("package p\nevent a.b: Order { key: id, headers: { tenant: string } } @deprecated\nevent c: { id: uuid }");
    let DeclKind::Event(e) = &f.decls[0].kind else {
        panic!()
    };
    assert_eq!(e.name.text(), "a.b");
    assert_eq!(e.options.len(), 2);
    assert_eq!(e.annotations[0].name.name, "deprecated");
    let DeclKind::Event(e) = &f.decls[1].kind else {
        panic!()
    };
    assert!(matches!(e.ty.kind, TypeKind::Inline(_)));
}

#[test]
fn http_routes_results_and_base_path() {
    let f = ok(r#"package p
http /v1 {
  POST "/orders" create(body: CreateOrder) -> Order | OutOfStock | InvalidInput
  GET "/orders/{id}" get(id: uuid) -> Order?
  GET "/orders"
    list(query: { page?: int }) -> Order[] @deprecated("use search")
}
http { GET "/x" x() -> Page<Order> }"#);
    let DeclKind::Http(h) = &f.decls[0].kind else {
        panic!()
    };
    assert_eq!(h.base.as_ref().unwrap().0, "/v1");
    assert_eq!(h.routes.len(), 3);
    assert!(matches!(&h.routes[0].result.ty.kind, TypeKind::Union(v) if v.len() == 3));
    assert!(matches!(h.routes[1].result.ty.kind, TypeKind::Optional(_)));
    assert_eq!(h.routes[2].annotations.len(), 1);
    let DeclKind::Http(h) = &f.decls[1].kind else {
        panic!()
    };
    assert!(h.base.is_none());
}

#[test]
fn rpc_streaming_and_unnamed_params() {
    let f =
        ok("package p\nrpc S { a(Req) -> Res  b(id: uuid) -> stream Res  c(stream Req) -> Res }");
    let DeclKind::Rpc(r) = &f.decls[0].kind else {
        panic!()
    };
    assert!(r.methods[0].params[0].name.is_none());
    assert!(r.methods[1].result.stream);
    assert!(r.methods[2].params[0].stream);
}

#[test]
fn graphql_and_sockets() {
    let f = ok("package p\ngraphql { query a(id: uuid) -> X? mutation b(input: Y) -> X subscription c() -> X }\nsocket s { server a.b: { id: uuid } client c: Y }");
    let DeclKind::Graphql(g) = &f.decls[0].kind else {
        panic!()
    };
    assert_eq!(g.fields.len(), 3);
    assert_eq!(g.fields[2].kind, GraphqlKind::Subscription);
    let DeclKind::Socket(s) = &f.decls[1].kind else {
        panic!()
    };
    assert_eq!(s.events[0].side, Side::Server);
    assert_eq!(s.events[1].side, Side::Client);
}

#[test]
fn enums_with_annotations_and_docs() {
    let f = ok("package p\nenum C { /// euro\n EUR GBP @deprecated, USD }");
    let DeclKind::Enum(e) = &f.decls[0].kind else {
        panic!()
    };
    assert_eq!(e.variants.len(), 3);
    assert_eq!(e.variants[0].doc, ["euro"]);
    assert_eq!(e.variants[1].annotations.len(), 1);
}

#[test]
fn error_decl_with_status() {
    let f = ok("package p\nerror PaymentDeclined @status(402) { reason: string retryable: bool }");
    let DeclKind::Error(e) = &f.decls[0].kind else {
        panic!()
    };
    assert_eq!(e.annotations[0].args, [AnnotationArg::Num("402".into())]);
}

#[test]
fn spans_point_at_the_source() {
    let src = "package p\ntype Money { amount: int }";
    let f = ok(src);
    let DeclKind::Type(t) = &f.decls[0].kind else {
        panic!()
    };
    assert_eq!(&src[t.name.span.clone()], "Money");
    assert_eq!(&src[t.fields[0].span.clone()], "amount: int");
    assert_eq!(&src[f.decls[0].span.clone()], "type Money { amount: int }");
}

// ---- errors ----

#[test]
fn error_missing_package() {
    insta::assert_snapshot!(err("type A { x: int }"));
}

#[test]
fn error_missing_colon() {
    insta::assert_snapshot!(err("package p\ntype A {\n  x int\n}"));
}

#[test]
fn error_unclosed_brace() {
    insta::assert_snapshot!(err("package p\ntype A {\n  x: int\n"));
}

#[test]
fn error_unknown_declaration() {
    insta::assert_snapshot!(err("package p\nmessage A { x: int }"));
}

#[test]
fn error_missing_arrow() {
    insta::assert_snapshot!(err("package p\nhttp {\n  GET \"/a\" a() Order\n}"));
}

#[test]
fn error_unterminated_string() {
    insta::assert_snapshot!(err("package p\nhttp {\n  GET \"/a a() -> Order\n}"));
}

#[test]
fn error_unexpected_character() {
    insta::assert_snapshot!(err("package p\ntype A { x: int; }"));
}

#[test]
fn error_bad_socket_side() {
    insta::assert_snapshot!(err("package p\nsocket s { both a: X }"));
}

#[test]
fn error_unknown_escape() {
    insta::assert_snapshot!(err("package p\ntype A { x: string @pattern(\"\\d\") }"));
}

#[test]
fn error_doc_comment_without_target() {
    insta::assert_snapshot!(err("package p\ntype A { x: int }\n/// dangling\n"));
}
