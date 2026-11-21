use weft_check::{check, Source};
use weft_ir::*;

const ORDERS: &str = include_str!("../../../../spec/examples/orders.weft");
const H: &str = "package p\n";

fn run(files: &[(&str, &str)]) -> Result<Vec<Ir>, Vec<miette::Report>> {
    let sources: Vec<Source> = files.iter().map(|(n, t)| Source::new(*n, *t)).collect();
    check(&sources)
}

fn ok(src: &str) -> Ir {
    let mut irs = run(&[("t.weft", src)]).unwrap_or_else(|e| panic!("{}", render(&e)));
    irs.remove(0)
}

fn render(errors: &[miette::Report]) -> String {
    use miette::{GraphicalReportHandler, GraphicalTheme};
    let handler =
        GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor()).with_width(90);
    errors
        .iter()
        .map(|e| {
            let mut out = String::new();
            handler.render_report(&mut out, e.as_ref()).unwrap();
            out
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn messages(files: &[(&str, &str)]) -> Vec<String> {
    run(files)
        .expect_err("expected errors")
        .iter()
        .map(|e| e.to_string())
        .collect()
}

fn rendered(src: &str) -> String {
    render(&run(&[("orders.weft", src)]).expect_err("expected errors"))
}

macro_rules! rejects {
    ($name:ident, $src:expr, $needle:expr) => {
        #[test]
        fn $name() {
            let src = format!("{H}{}", $src);
            let m = messages(&[("t.weft", &src)]);
            assert!(
                m.iter().any(|m| m.contains($needle)),
                "wanted {:?} in {m:#?}",
                $needle
            );
        }
    };
}

fn validates(ir: &Ir) {
    let schema: serde_json::Value = serde_json::from_str(&json_schema()).unwrap();
    let v = jsonschema::validator_for(&schema).unwrap();
    let value: serde_json::Value = serde_json::from_str(&ir.to_json()).unwrap();
    let errors: Vec<String> = v.iter_errors(&value).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
}

// ---- the complete example ----

#[test]
fn the_complete_example_compiles_and_matches_the_schema() {
    let ir = ok(ORDERS);
    validates(&ir);
    insta::assert_snapshot!(ir.to_json());
}

#[test]
fn example_results_are_split_into_value_and_errors() {
    let ir = ok(ORDERS);
    let create = ir.routes.iter().find(|r| r.name == "createOrder").unwrap();
    assert_eq!(create.result.errors.len(), 2);
    assert_eq!(
        create.result.errors[0],
        ErrorRef {
            package: "orders".into(),
            name: "OutOfStock".into()
        }
    );
    assert_eq!(
        create.result.errors[1],
        ErrorRef {
            package: "weft".into(),
            name: "InvalidInput".into()
        }
    );
    let get = ir.routes.iter().find(|r| r.name == "getOrder").unwrap();
    assert!(get.result.optional);
    assert_eq!(get.path_params[0].name, "id");
    assert_eq!(get.base.as_deref(), Some("/v1"));
    let updates = &ir.rpc_services[0].methods[1];
    assert!(updates.result.stream);
    assert_eq!(ir.events[0].key, ["id"]);
}

#[test]
fn inline_types_get_generated_names() {
    let ir = ok(ORDERS);
    let names: Vec<&str> = ir
        .types
        .iter()
        .filter(|t| t.inline)
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "OrderCancelled",
            "ListOrdersQuery",
            "StreamUpdatesRequest",
            "CourierMoved",
            "TrackStart"
        ]
    );
}

// ---- positive cases ----

#[test]
fn types_may_be_used_before_they_are_declared() {
    let ir = ok(&format!("{H}type A {{ b: B }}\ntype B {{ x: int }}"));
    assert_eq!(ir.types.len(), 2);
}

#[test]
fn generics_maps_and_lists_resolve() {
    let ir = ok(&format!("{H}type Page<T> {{ items: T[] next?: string }}\nenum K {{ A }}\ntype X {{ p: Page<int> m: map<K, Page<string>[]> }}"));
    let TypeBody::Record { fields } = &ir.types[2].body else {
        panic!()
    };
    assert!(matches!(fields[0].ty, TypeRef::Named { ref args, .. } if args.len() == 1));
    assert!(matches!(fields[1].ty, TypeRef::Map { .. }));
}

#[test]
fn wire_names_validation_rules_and_deprecation() {
    let ir = ok(&format!(
        r#"{H}type T {{
  a: int @min(1) @max(100) @wire("a_wire")
  b: string @length(1, 20) @pattern("^[a-z]+$") @format(email) @deprecated("use a")
  c: string[] @length(0, 5)
}}"#
    ));
    let TypeBody::Record { fields } = &ir.types[0].body else {
        panic!()
    };
    assert_eq!(fields[0].wire_name, "a_wire");
    assert_eq!(fields[0].rules.len(), 2);
    assert_eq!(fields[1].rules.len(), 3);
    assert_eq!(fields[1].deprecated.as_deref(), Some("use a"));
}

#[test]
fn error_status_defaults_to_422_and_can_be_set() {
    let ir = ok(&format!(
        "{H}error A {{ x: int }}\nerror B @status(402) {{ x: int }}"
    ));
    assert_eq!(ir.errors[0].status, 422);
    assert_eq!(ir.errors[1].status, 402);
}

#[test]
fn keys_may_walk_into_nested_records() {
    let ir = ok(&format!(
        "{H}type C {{ id: uuid }}\ntype O {{ c: C }}\nevent e.f: O {{ key: c.id }}"
    ));
    assert_eq!(ir.events[0].key, ["c", "id"]);
}

#[test]
fn rpc_named_parameters_become_a_request_record() {
    let ir = ok(&format!(
        "{H}rpc S {{ get(id: uuid, n?: int) -> X }}\ntype X {{ a: int }}"
    ));
    let m = &ir.rpc_services[0].methods[0];
    assert!(matches!(&m.request, TypeRef::Named { name, .. } if name == "GetRequest"));
    let req = ir.types.iter().find(|t| t.name == "GetRequest").unwrap();
    let TypeBody::Record { fields } = &req.body else {
        panic!()
    };
    assert_eq!(fields.len(), 2);
    assert!(fields[1].optional);
}

#[test]
fn graphql_query_and_mutation_may_share_a_name() {
    ok(&format!(
        "{H}type X {{ a: int }}\ngraphql {{ query a() -> X mutation a() -> X }}"
    ));
}

#[test]
fn files_of_one_package_are_merged() {
    let irs = run(&[
        ("a.weft", "package p\ntype A { b: B }"),
        ("b.weft", "package p\ntype B { x: int }\nevent e: A"),
    ])
    .unwrap();
    assert_eq!(irs.len(), 1);
    assert_eq!(irs[0].types.len(), 2);
    assert_eq!(irs[0].events.len(), 1);
}

#[test]
fn packages_import_each_other() {
    let irs = run(&[
        ("orders.weft", ORDERS),
        (
            "payments.weft",
            "package payments @version(\"0.3.0\")\nimport orders\nevent payment.captured: { order: orders.Order, amount: orders.Money }\nhttp { POST \"/pay\" pay(body: orders.CreateOrder) -> orders.Order | orders.OutOfStock }",
        ),
    ])
    .unwrap();
    assert_eq!(irs.len(), 2);
    let payments = irs.iter().find(|i| i.package == "payments").unwrap();
    assert_eq!(payments.imports, ["orders"]);
    assert_eq!(payments.version.as_deref(), Some("0.3.0"));
    validates(payments);
    let TypeBody::Record { fields } = &payments.types[0].body else {
        panic!()
    };
    assert!(matches!(&fields[0].ty, TypeRef::Named { package, .. } if package == "orders"));
    assert_eq!(payments.routes[0].result.errors[0].package, "orders");
}

// ---- packages ----

#[test]
fn error_package_named_weft() {
    let m = messages(&[("t.weft", "package weft")]);
    assert!(m[0].contains("reserved"));
}

#[test]
fn error_unknown_import() {
    let m = messages(&[("t.weft", "package p\nimport nope")]);
    assert!(m[0].contains("unknown package `nope`"));
}

#[test]
fn error_self_import() {
    let m = messages(&[("t.weft", "package p\nimport p")]);
    assert!(m[0].contains("imports itself"));
}

#[test]
fn error_import_cycle() {
    let m = messages(&[
        ("a.weft", "package a\nimport b"),
        ("b.weft", "package b\nimport a"),
    ]);
    assert!(m.iter().any(|m| m.contains("import cycle")), "{m:?}");
}

#[test]
fn error_duplicate_declaration_across_files() {
    let m = messages(&[
        ("a.weft", "package p\ntype A { x: int }"),
        ("b.weft", "package p\nenum A { X }"),
    ]);
    assert!(m[0].contains("declared twice"));
}

#[test]
fn error_two_versions() {
    let m = messages(&[
        ("a.weft", "package p @version(\"1.0.0\")"),
        ("b.weft", "package p @version(\"2.0.0\")"),
    ]);
    assert!(m[0].contains("two versions"));
}

#[test]
fn error_bad_version() {
    let m = messages(&[("t.weft", "package p @version(\"1.0\")")]);
    assert!(m[0].contains("not a semantic version"));
}

#[test]
fn error_type_from_package_not_imported() {
    let m = messages(&[
        ("a.weft", "package a\ntype X { y: int }"),
        ("b.weft", "package b\ntype Z { x: a.X }"),
    ]);
    assert!(m[0].contains("not imported"));
}

rejects!(
    error_own_package_qualified,
    "type A { x: int }\ntype B { a: p.A }",
    "this package"
);
rejects!(
    error_too_many_name_parts,
    "type B { a: x.y.z }",
    "at most two parts"
);

#[test]
fn error_unknown_member_of_imported_package() {
    let m = messages(&[
        ("a.weft", "package a\ntype Money { x: int }"),
        ("b.weft", "package b\nimport a\ntype Z { x: a.Mony }"),
    ]);
    assert!(m[0].contains("has no type `Mony`"));
}

// ---- types ----

rejects!(
    error_unknown_type,
    "type A { x: Strng }",
    "unknown type `Strng`"
);
rejects!(
    error_generic_arity,
    "type P<T> { x: T }\ntype A { x: P }",
    "takes 1 type argument"
);
rejects!(
    error_not_generic,
    "type B { x: int }\ntype A { x: B<int> }",
    "is not generic"
);
rejects!(
    error_primitive_with_args,
    "type A { x: int<string> }",
    "takes no type arguments"
);
rejects!(
    error_map_arg_count,
    "type A { x: map<string> }",
    "key type and a value type"
);
rejects!(
    error_map_key,
    "type A { x: map<int, string> }",
    "cannot be a map key"
);
rejects!(
    error_error_used_as_type,
    "error E { a: int }\ntype A { x: E }",
    "is an error, not a type"
);
rejects!(
    error_builtin_error_used_as_type,
    "type A { x: NotFound }",
    "is an error, not a type"
);
rejects!(
    error_duplicate_field,
    "type A { x: int x: string }",
    "declared twice"
);
rejects!(
    error_duplicate_wire_name,
    "type A { x: int @wire(\"y\") y: int }",
    "called `y` on the wire"
);
rejects!(
    error_inline_in_generic,
    "type P<T> { x: { a: T } }",
    "inline types are not allowed inside a generic"
);
rejects!(
    error_duplicate_type_param,
    "type P<T, T> { x: T }",
    "type parameter `T` is declared twice"
);
rejects!(
    error_builtin_name_for_type,
    "type string { x: int }",
    "built-in name"
);
rejects!(
    error_builtin_name_for_error,
    "error NotFound { x: int }",
    "built-in name"
);
rejects!(error_empty_enum, "enum E { }", "has no values");
rejects!(error_duplicate_variant, "enum E { A A }", "appears twice");
rejects!(
    error_optional_type_outside_result,
    "type X { y: int }\ntype Q { z: X? }",
    "expected"
);

// ---- annotations ----

rejects!(
    error_unknown_annotation,
    "type A { x: int @mni(1) }",
    "unknown annotation `@mni`"
);
rejects!(
    error_annotation_wrong_target,
    "type A @min(1) { x: int }",
    "cannot be used on a type"
);
rejects!(
    error_duplicate_annotation,
    "type A { x: int @min(1) @min(2) }",
    "used twice"
);
rejects!(
    error_min_on_string,
    "type A { x: string @min(1) }",
    "applies to numbers"
);
rejects!(
    error_min_fraction_on_int,
    "type A { x: int @min(0.5) }",
    "whole number"
);
rejects!(
    error_max_below_min,
    "type A { x: int @min(5) @max(1) }",
    "below `@min`"
);
rejects!(
    error_length_order,
    "type A { x: string @length(5, 1) }",
    "above its maximum"
);
rejects!(
    error_length_on_int,
    "type A { x: int @length(1, 2) }",
    "applies to strings, lists and maps"
);
rejects!(
    error_pattern_lookahead,
    "type A { x: string @pattern(\"^(?=a)\") }",
    "regex subset"
);
rejects!(
    error_pattern_backreference,
    "type A { x: string @pattern(\"(a)\\\\1\") }",
    "regex subset"
);
rejects!(
    error_pattern_on_int,
    "type A { x: int @pattern(\"a\") }",
    "applies to strings"
);
rejects!(
    error_unknown_format,
    "type A { x: string @format(emal) }",
    "unknown format"
);
rejects!(
    error_format_on_int,
    "type A { x: int @format(email) }",
    "applies to strings"
);
rejects!(
    error_status_out_of_range,
    "error E @status(200) { x: int }",
    "not an HTTP error status"
);
rejects!(
    error_deprecated_args,
    "type A { x: int @deprecated(1) }",
    "takes an optional message"
);
rejects!(
    error_wire_empty,
    "type A { x: int @wire(\"\") }",
    "takes a name"
);
rejects!(
    error_min_without_args,
    "type A { x: int @min }",
    "takes a number"
);

// ---- events ----

rejects!(
    error_duplicate_event,
    "event a: { x: int }\nevent a: { x: int }",
    "declared twice"
);
rejects!(
    error_event_payload_enum,
    "enum E { A }\nevent a: E",
    "must be a record type"
);
rejects!(
    error_event_payload_list,
    "type A { x: int }\nevent a: A[]",
    "must be a record type"
);
rejects!(
    error_key_missing_field,
    "type A { id: int }\nevent a: A { key: idd }",
    "no field `idd`"
);
rejects!(
    error_key_optional,
    "type A { id?: int }\nevent a: A { key: id }",
    "optional"
);
rejects!(
    error_key_bad_type,
    "type A { f: float }\nevent a: A { key: f }",
    "must be a string"
);
rejects!(
    error_two_keys,
    "type A { id: int }\nevent a: A { key: id key: id }",
    "has one `key`"
);
rejects!(
    error_headers_not_strings,
    "type A { id: int }\nevent a: A { headers: { n: int } }",
    "can only hold `string` fields"
);

// ---- http ----

const X: &str = "type X { a: int }\n";

macro_rules! rejects_http {
    ($name:ident, $route:expr, $needle:expr) => {
        #[test]
        fn $name() {
            let src = format!("{H}{X}http {{ {} }}", $route);
            let m = messages(&[("t.weft", &src)]);
            assert!(
                m.iter().any(|m| m.contains($needle)),
                "wanted {:?} in {m:#?}",
                $needle
            );
        }
    };
}

rejects_http!(
    error_http_bad_method,
    "FETCH \"/a\" a() -> X",
    "not an HTTP method"
);
rejects_http!(
    error_http_path_without_slash,
    "GET \"a\" a() -> X",
    "starts with `/`"
);
rejects_http!(
    error_http_unclosed_brace,
    "GET \"/a/{id\" a() -> X",
    "unclosed"
);
rejects_http!(
    error_http_undeclared_path_param,
    "GET \"/a/{id}\" a() -> X",
    "is not declared"
);
rejects_http!(
    error_http_param_not_in_path,
    "GET \"/a\" a(id: uuid) -> X",
    "is not in the path"
);
rejects_http!(
    error_http_optional_path_param,
    "GET \"/a/{id}\" a(id?: uuid) -> X",
    "required scalar"
);
rejects_http!(
    error_http_record_path_param,
    "GET \"/a/{id}\" a(id: X) -> X",
    "required scalar"
);
rejects_http!(
    error_http_get_with_body,
    "GET \"/a\" a(body: X) -> X",
    "cannot have a body"
);
rejects_http!(
    error_http_unnamed_param,
    "GET \"/a\" a(X) -> X",
    "needs a name"
);
rejects_http!(
    error_http_optional_body,
    "POST \"/a\" a(body?: X) -> X",
    "cannot be optional"
);
rejects_http!(
    error_http_query_not_record,
    "GET \"/a\" a(query: int) -> X",
    "must be a record type"
);
rejects_http!(
    error_http_stream,
    "GET \"/a\" a() -> stream X",
    "only for `rpc`"
);
rejects_http!(
    error_http_duplicate_name,
    "GET \"/a\" a() -> X\nGET \"/b\" a() -> X",
    "declared twice"
);
rejects_http!(
    error_http_duplicate_route,
    "GET \"/a/{x}\" a(x: int) -> X\nGET \"/a/{y}\" b(y: int) -> X",
    "already handles"
);
rejects_http!(
    error_http_error_first,
    "GET \"/a\" a() -> NotFound | X",
    "is an error, not a type"
);
rejects_http!(
    error_http_value_after_value,
    "GET \"/a\" a() -> X | X",
    "is not an error"
);
rejects_http!(
    error_http_optional_error,
    "GET \"/a\" a() -> X | NotFound?",
    "only the result value can be optional"
);
rejects_http!(
    error_http_duplicate_error,
    "GET \"/a\" a() -> X | NotFound | NotFound",
    "listed twice"
);

#[test]
fn error_http_base_path_with_params() {
    let m = messages(&[("t.weft", "package p\nhttp /v1/{x} { }")]);
    assert!(m[0].contains("base path"));
}

// ---- rpc, graphql, sockets ----

rejects!(
    error_rpc_duplicate_service,
    "type X { a: int }\nrpc S { a(X) -> X }\nrpc S { b(X) -> X }",
    "declared twice"
);
rejects!(
    error_rpc_duplicate_method,
    "type X { a: int }\nrpc S { a(X) -> X a(X) -> X }",
    "declared twice"
);
rejects!(
    error_rpc_mixed_params,
    "type X { a: int }\nrpc S { a(X, n: int) -> X }",
    "mixes named and unnamed"
);
rejects!(
    error_rpc_request_not_record,
    "type X { a: int }\nrpc S { a(int) -> X }",
    "must be a record type"
);
rejects!(
    error_graphql_unnamed_arg,
    "type X { a: int }\ngraphql { query a(X) -> X }",
    "needs a name"
);
rejects!(
    error_graphql_duplicate,
    "type X { a: int }\ngraphql { query a() -> X query a() -> X }",
    "declared twice"
);
rejects!(
    error_graphql_stream,
    "type X { a: int }\ngraphql { subscription a() -> stream X }",
    "only for `rpc`"
);
rejects!(
    error_socket_duplicate,
    "socket s { }\nsocket s { }",
    "declared twice"
);
rejects!(
    error_socket_duplicate_event,
    "socket s { server a: { x: int } client a: { x: int } }",
    "declared twice"
);
rejects!(
    error_socket_payload_not_record,
    "socket s { server a: int }",
    "must be a record type"
);
rejects!(
    error_inline_name_clash,
    "type OrderPlaced { x: int }\nevent order.placed: { y: int }",
    "already used"
);

// ---- how errors read ----

#[test]
fn snapshot_unknown_type_with_suggestion() {
    insta::assert_snapshot!(rendered(
        "package orders\ntype Order {\n  total: Mony\n}\ntype Money { amount: int }"
    ));
}

#[test]
fn snapshot_param_not_in_path() {
    insta::assert_snapshot!(rendered(
        "package orders\ntype O { id: uuid }\nhttp {\n  GET \"/orders\" list(page: int) -> O[]\n}"
    ));
}

#[test]
fn snapshot_several_errors_in_one_run() {
    insta::assert_snapshot!(rendered(
        "package orders\ntype A { x: Nope y: int @min(\"a\") }\nevent a: A { key: zzz }"
    ));
}

#[test]
fn snapshot_pattern_lookahead() {
    insta::assert_snapshot!(rendered(
        "package orders\ntype A { x: string @pattern(\"^(?=a)\") }"
    ));
}

#[test]
fn syntax_errors_come_through() {
    let m = messages(&[("t.weft", "package p\ntype A { x int }")]);
    assert!(m[0].contains("expected"));
}

#[test]
fn nothing_is_returned_when_any_file_has_errors() {
    let r = run(&[
        ("a.weft", "package a\ntype A { x: int }"),
        ("b.weft", "package b\ntype B { x: Nope }"),
    ]);
    assert!(r.is_err());
}
