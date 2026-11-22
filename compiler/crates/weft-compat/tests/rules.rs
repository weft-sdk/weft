//! One test per row of "Evolving contracts safely" in docs/02-contract-language.md, then the cases the table does not
//! list, then the version rules.

use weft_check::{check, Source};
use weft_compat::{compare, compare_package, Bump, PackageReport, Severity};
use weft_ir::Ir;

fn ir(src: &str) -> Ir {
    let mut irs = check(&[Source::new("t.weft", src)]).unwrap_or_else(|e| panic!("{e:?}\n{src}"));
    irs.remove(0)
}

fn report(old: &str, new: &str) -> PackageReport {
    compare_package(&ir(old), &ir(new))
}

fn worst(r: &PackageReport) -> Option<Severity> {
    r.changes.iter().map(|c| c.severity).max()
}

#[track_caller]
fn expect(old: &str, new: &str, severity: Severity, needle: &str) {
    let r = report(old, new);
    assert!(
        r.changes.iter().any(|c| c.severity == severity
            && (c.message.contains(needle) || c.location.contains(needle))),
        "wanted a {severity:?} change mentioning {needle:?}, got:\n{:#?}",
        r.changes
    );
}

#[track_caller]
fn expect_worst(old: &str, new: &str, severity: Severity) {
    let r = report(old, new);
    assert_eq!(worst(&r), Some(severity), "{:#?}", r.changes);
}

#[track_caller]
fn expect_nothing(old: &str, new: &str) {
    let r = report(old, new);
    assert!(r.changes.is_empty(), "{:#?}", r.changes);
}

// A type used only in requests, only in events or responses, and in both.
fn input(fields: &str) -> String {
    format!("package p\ntype R {{ ok: bool }}\ntype O {{ {fields} }}\nhttp {{ POST \"/o\" create(body: O) -> R }}")
}
fn output(fields: &str) -> String {
    format!("package p\ntype O {{ {fields} }}\nevent o.made: O")
}
fn response(fields: &str) -> String {
    format!("package p\ntype O {{ {fields} }}\nhttp {{ GET \"/o\" get() -> O }}")
}
fn both(fields: &str) -> String {
    format!("package p\ntype O {{ {fields} }}\nhttp {{ POST \"/o\" create(body: O) -> O }}")
}
fn unused(fields: &str) -> String {
    format!("package p\ntype O {{ {fields} }}")
}

// ---- the table ----

#[test]
fn row_add_an_optional_field_is_safe() {
    for f in [input, output, both, unused] {
        expect_worst(&f("id: uuid"), &f("id: uuid note?: string"), Severity::Safe);
    }
}

#[test]
fn row_add_a_required_field_to_an_event_or_response_is_safe_for_receivers() {
    expect(
        &output("id: uuid"),
        &output("id: uuid n: int"),
        Severity::Safe,
        "added required field `n`",
    );
    expect(
        &response("id: uuid"),
        &response("id: uuid n: int"),
        Severity::Safe,
        "added required field `n`",
    );
    expect_worst(
        &output("id: uuid"),
        &output("id: uuid n: int"),
        Severity::Safe,
    );
}

#[test]
fn row_add_a_required_field_to_a_request_or_body_is_breaking() {
    expect(
        &input("id: uuid"),
        &input("id: uuid n: int"),
        Severity::Breaking,
        "added required field `n`",
    );
}

#[test]
fn a_required_field_on_a_type_used_both_ways_or_by_nothing_is_breaking() {
    expect(
        &both("id: uuid"),
        &both("id: uuid n: int"),
        Severity::Breaking,
        "added required field",
    );
    expect(
        &unused("id: uuid"),
        &unused("id: uuid n: int"),
        Severity::Breaking,
        "added required field",
    );
}

#[test]
fn row_remove_a_field_is_breaking() {
    for f in [input, output, both, unused] {
        expect(
            &f("id: uuid n: int"),
            &f("id: uuid"),
            Severity::Breaking,
            "removed field `n`",
        );
    }
}

#[test]
fn row_rename_a_field_is_breaking() {
    let r = report(&output("id: uuid n: int"), &output("id: uuid count: int"));
    assert_eq!(worst(&r), Some(Severity::Breaking));
    assert!(r
        .changes
        .iter()
        .any(|c| c.message.contains("removed field `n`")));
    assert!(r
        .changes
        .iter()
        .any(|c| c.severity == Severity::Breaking
            && c.hint.as_deref().unwrap().contains("@deprecated")));
}

#[test]
fn row_change_a_fields_type_is_breaking() {
    for f in [input, output, both] {
        expect(
            &f("n: int"),
            &f("n: string"),
            Severity::Breaking,
            "type of `n` changed",
        );
        expect(
            &f("n: string"),
            &f("n: uuid"),
            Severity::Breaking,
            "type of `n` changed",
        );
    }
}

#[test]
fn row_widening_a_type_is_risky() {
    expect(
        &output("n: int32"),
        &output("n: int"),
        Severity::Risky,
        "widened",
    );
    expect(
        &output("n: int32"),
        &output("n: int64"),
        Severity::Risky,
        "widened",
    );
    expect(
        &output("n: float32"),
        &output("n: float64"),
        Severity::Risky,
        "widened",
    );
    // The other way round is narrowing.
    expect(
        &output("n: int"),
        &output("n: int32"),
        Severity::Breaking,
        "type of `n`",
    );
}

#[test]
fn int_and_int64_are_the_same_type() {
    expect_nothing(&output("n: int"), &output("n: int64"));
    expect_nothing(&output("n: float"), &output("n: float64"));
}

#[test]
fn row_make_a_required_field_optional_is_breaking_for_receivers() {
    expect(
        &output("n: int"),
        &output("n?: int"),
        Severity::Breaking,
        "now optional",
    );
    expect(
        &response("n: int"),
        &response("n?: int"),
        Severity::Breaking,
        "now optional",
    );
    expect(
        &both("n: int"),
        &both("n?: int"),
        Severity::Breaking,
        "now optional",
    );
    // Callers lose nothing when a request field becomes optional.
    expect(
        &input("n: int"),
        &input("n?: int"),
        Severity::Safe,
        "now optional",
    );
}

#[test]
fn making_an_optional_field_required_is_breaking_for_senders() {
    expect(
        &input("n?: int"),
        &input("n: int"),
        Severity::Breaking,
        "now required",
    );
    expect(
        &output("n?: int"),
        &output("n: int"),
        Severity::Safe,
        "now required",
    );
}

#[test]
fn row_add_an_enum_value_is_risky() {
    let old = "package p\nenum C { A B }\ntype O { c: C }\nevent o.made: O";
    let new = "package p\nenum C { A B D }\ntype O { c: C }\nevent o.made: O";
    expect(old, new, Severity::Risky, "added value `D`");
    expect_worst(old, new, Severity::Risky);
}

#[test]
fn removing_an_enum_value_is_breaking() {
    let old = "package p\nenum C { A B }\ntype O { c: C }\nevent o.made: O";
    let new = "package p\nenum C { A }\ntype O { c: C }\nevent o.made: O";
    expect(old, new, Severity::Breaking, "removed value `B`");
}

#[test]
fn row_remove_an_operation_is_breaking() {
    let base = "package p\ntype X { a: int }\n";
    let cases = [
        (
            "http { GET \"/a\" a() -> X GET \"/b\" b() -> X }",
            "http { GET \"/a\" a() -> X }",
            "removed route `b`",
        ),
        ("event a: X\nevent b: X", "event a: X", "removed event `b`"),
        (
            "rpc S { a(X) -> X b(X) -> X }",
            "rpc S { a(X) -> X }",
            "removed method `b`",
        ),
        (
            "rpc S { a(X) -> X }\nrpc T { a(X) -> X }",
            "rpc S { a(X) -> X }",
            "removed rpc service `T`",
        ),
        (
            "graphql { query a() -> X query b() -> X }",
            "graphql { query a() -> X }",
            "removed `b`",
        ),
        (
            "socket s { server a: X server b: X }",
            "socket s { server a: X }",
            "removed event `b`",
        ),
        (
            "socket s { server a: X }\nsocket t { server a: X }",
            "socket s { server a: X }",
            "removed socket `t`",
        ),
    ];
    for (old, new, needle) in cases {
        expect(
            &format!("{base}{old}"),
            &format!("{base}{new}"),
            Severity::Breaking,
            needle,
        );
    }
}

#[test]
fn adding_an_operation_is_safe() {
    let base = "package p\ntype X { a: int }\n";
    let cases = [
        (
            "http { GET \"/a\" a() -> X }",
            "http { GET \"/a\" a() -> X GET \"/b\" b() -> X }",
        ),
        ("event a: X", "event a: X\nevent b: X"),
        ("rpc S { a(X) -> X }", "rpc S { a(X) -> X b(X) -> X }"),
        (
            "graphql { query a() -> X }",
            "graphql { query a() -> X query b() -> X }",
        ),
        (
            "socket s { server a: X }",
            "socket s { server a: X server b: X }",
        ),
    ];
    for (old, new) in cases {
        expect_worst(
            &format!("{base}{old}"),
            &format!("{base}{new}"),
            Severity::Safe,
        );
    }
}

#[test]
fn row_change_an_events_key_is_breaking() {
    let old = "package p\ntype X { id: uuid other: uuid }\nevent a: X { key: id }";
    let new = "package p\ntype X { id: uuid other: uuid }\nevent a: X { key: other }";
    expect(old, new, Severity::Breaking, "routing key changed");
    let none = "package p\ntype X { id: uuid other: uuid }\nevent a: X";
    expect(old, none, Severity::Breaking, "routing key changed");
}

#[test]
fn row_tighten_validation_is_breaking_for_senders() {
    for (old, new) in [
        ("n: int @max(100)", "n: int @max(50)"),
        ("n: int @min(1)", "n: int @min(5)"),
        ("n: int", "n: int @max(5)"),
        ("s: string @length(1, 20)", "s: string @length(2, 20)"),
        ("s: string @length(1, 20)", "s: string @length(1, 10)"),
        ("s: string", "s: string @pattern(\"^a\")"),
        ("s: string @pattern(\"^a\")", "s: string @pattern(\"^b\")"),
        ("s: string", "s: string @format(email)"),
    ] {
        expect(
            &input(old),
            &input(new),
            Severity::Breaking,
            "validation tightened",
        );
        // Receivers are not hurt by it.
        expect_worst(&output(old), &output(new), Severity::Safe);
    }
}

#[test]
fn row_loosen_validation_is_safe_for_senders_and_risky_for_receivers() {
    for (old, new) in [
        ("n: int @max(50)", "n: int @max(100)"),
        ("n: int @min(5)", "n: int @min(1)"),
        ("n: int @max(5)", "n: int"),
        ("s: string @length(2, 10)", "s: string @length(1, 20)"),
        ("s: string @pattern(\"^a\")", "s: string"),
        ("s: string @format(email)", "s: string"),
    ] {
        expect_worst(&input(old), &input(new), Severity::Safe);
        expect(
            &output(old),
            &output(new),
            Severity::Risky,
            "validation loosened",
        );
    }
}

// ---- beyond the table ----

#[test]
fn changing_a_wire_name_is_breaking() {
    expect(
        &output("n: int @wire(\"a\")"),
        &output("n: int @wire(\"b\")"),
        Severity::Breaking,
        "wire name",
    );
}

#[test]
fn removing_a_type_is_breaking_but_adding_one_is_safe() {
    expect(
        "package p\ntype A { x: int }\ntype B { x: int }",
        "package p\ntype A { x: int }",
        Severity::Breaking,
        "removed type `B`",
    );
    expect(
        "package p\ntype A { x: int }",
        "package p\ntype A { x: int }\ntype B { x: int }",
        Severity::Safe,
        "added type `B`",
    );
}

#[test]
fn changing_between_a_record_and_an_enum_is_breaking() {
    expect(
        "package p\ntype A { x: int }",
        "package p\nenum A { X }",
        Severity::Breaking,
        "record and an enum",
    );
}

#[test]
fn deprecating_is_safe() {
    expect_worst(
        &output("n: int"),
        &output("n: int @deprecated(\"use m\")"),
        Severity::Safe,
    );
    expect(
        "package p\ntype A { x: int }",
        "package p\ntype A @deprecated { x: int }",
        Severity::Safe,
        "now deprecated",
    );
}

#[test]
fn generic_arguments_carry_direction() {
    // Order is only ever returned, inside Page<Order>, so a new required field is safe for receivers.
    let page = "type Page<T> { items: T[] }\n";
    let old = format!(
        "package p\n{page}type Order {{ id: uuid }}\nhttp {{ GET \"/o\" list() -> Page<Order> }}"
    );
    let new = format!("package p\n{page}type Order {{ id: uuid n: int }}\nhttp {{ GET \"/o\" list() -> Page<Order> }}");
    expect_worst(&old, &new, Severity::Safe);
    // Changing the argument is a type change.
    let other = format!("package p\n{page}type Order {{ id: uuid }}\ntype Other {{ id: uuid }}\nhttp {{ GET \"/o\" list() -> Page<Other> }}");
    expect(&old, &other, Severity::Breaking, "result changed");
}

#[test]
fn nested_types_inherit_direction() {
    let old = "package p\ntype Inner { a: int }\ntype Outer { i: Inner }\nhttp { POST \"/o\" c(body: Outer) -> Outer }";
    let new = "package p\ntype Inner { a: int b: int }\ntype Outer { i: Inner }\nhttp { POST \"/o\" c(body: Outer) -> Outer }";
    expect(old, new, Severity::Breaking, "added required field `b`");
}

#[test]
fn errors() {
    let base = "package p\ntype X { a: int }\n";
    let old = format!("{base}error E {{ code: int }}\nhttp {{ GET \"/a\" a() -> X | E }}");
    // removing, status, fields
    let removed = format!("{base}http {{ GET \"/a\" a() -> X }}");
    expect(&old, &removed, Severity::Breaking, "removed error `E`");
    let status =
        format!("{base}error E @status(409) {{ code: int }}\nhttp {{ GET \"/a\" a() -> X | E }}");
    expect(
        &old,
        &status,
        Severity::Breaking,
        "HTTP status of `E` changed from 422 to 409",
    );
    let field =
        format!("{base}error E {{ code: int why: string }}\nhttp {{ GET \"/a\" a() -> X | E }}");
    expect_worst(&old, &field, Severity::Safe);
    let dropped = format!("{base}error E {{ }}\nhttp {{ GET \"/a\" a() -> X | E }}");
    expect(&old, &dropped, Severity::Breaking, "removed field `code`");
}

#[test]
fn results() {
    let base = "package p\ntype X { a: int }\ntype Y { a: int }\n";
    let r = |res: &str| format!("{base}http {{ GET \"/a\" a() -> {res} }}");
    expect(&r("X"), &r("Y"), Severity::Breaking, "result changed");
    expect(&r("X"), &r("X?"), Severity::Breaking, "may now be empty");
    expect(&r("X?"), &r("X"), Severity::Safe, "no longer optional");
    expect(
        &r("X"),
        &r("X | NotFound"),
        Severity::Risky,
        "may now get the error `NotFound`",
    );
    expect(
        &r("X | NotFound"),
        &r("X"),
        Severity::Safe,
        "no longer returned",
    );
    expect(&r("X"), &r("X[]"), Severity::Breaking, "result changed");
    let rpc = |res: &str| format!("{base}rpc S {{ a(X) -> {res} }}");
    expect(
        &rpc("X"),
        &rpc("stream X"),
        Severity::Breaking,
        "now a stream",
    );
    expect(
        &rpc("stream X"),
        &rpc("X"),
        Severity::Breaking,
        "no longer a stream",
    );
}

#[test]
fn routes() {
    let base = "package p\ntype X { a: int }\n";
    let r = |route: &str| format!("{base}http /v1 {{ {route} }}");
    expect(
        &r("GET \"/a\" a() -> X"),
        &r("POST \"/a\" a() -> X"),
        Severity::Breaking,
        "method changed",
    );
    expect(
        &r("GET \"/a\" a() -> X"),
        &r("GET \"/b\" a() -> X"),
        Severity::Breaking,
        "path changed from `/v1/a` to `/v1/b`",
    );
    expect(
        &format!("{base}http /v1 {{ GET \"/a\" a() -> X }}"),
        &format!("{base}http /v2 {{ GET \"/a\" a() -> X }}"),
        Severity::Breaking,
        "path changed",
    );
    // path parameters
    expect(
        &r("GET \"/a/{id}\" a(id: uuid) -> X"),
        &r("GET \"/a/{id}\" a(id: string) -> X"),
        Severity::Breaking,
        "type of parameter `id`",
    );
    expect(
        &r("GET \"/a/{id}\" a(id: uuid) -> X"),
        &r("GET \"/a/{id}/{n}\" a(id: uuid, n: int) -> X"),
        Severity::Breaking,
        "added required parameter `n`",
    );
    // query, body, headers
    expect(
        &r("GET \"/a\" a() -> X"),
        &r("GET \"/a\" a(query: { p?: int }) -> X"),
        Severity::Safe,
        "only optional fields",
    );
    expect(
        &r("GET \"/a\" a() -> X"),
        &r("GET \"/a\" a(query: { p: int }) -> X"),
        Severity::Breaking,
        "added `query`",
    );
    expect(
        &r("POST \"/a\" a() -> X"),
        &r("POST \"/a\" a(body: X) -> X"),
        Severity::Breaking,
        "added `body`",
    );
    expect(
        &r("POST \"/a\" a(body: X) -> X"),
        &r("POST \"/a\" a() -> X"),
        Severity::Breaking,
        "removed `body`",
    );
    expect(
        &r("GET \"/a\" a(query: { p?: int }) -> X"),
        &r("GET \"/a\" a(query: { p?: int, q?: int }) -> X"),
        Severity::Safe,
        "added optional field `q`",
    );
    expect(
        &r("GET \"/a\" a(query: { p?: int }) -> X"),
        &r("GET \"/a\" a(query: { p?: int, q: int }) -> X"),
        Severity::Breaking,
        "added required field `q`",
    );
}

#[test]
fn rpc_and_graphql_and_sockets() {
    let base = "package p\ntype X { a: int }\ntype Y { a: int }\n";
    let rpc = |m: &str| format!("{base}rpc S {{ {m} }}");
    expect(
        &rpc("a(X) -> X"),
        &rpc("a(Y) -> X"),
        Severity::Breaking,
        "request changed",
    );
    expect(
        &rpc("a(X) -> X"),
        &rpc("a(stream X) -> X"),
        Severity::Breaking,
        "client streaming",
    );
    let gql = |m: &str| format!("{base}graphql {{ {m} }}");
    expect(
        &gql("query a(id: uuid) -> X"),
        &gql("query a(id: uuid, n?: int) -> X"),
        Severity::Safe,
        "added optional parameter `n`",
    );
    expect(
        &gql("query a(id: uuid) -> X"),
        &gql("query a(id: uuid, n: int) -> X"),
        Severity::Breaking,
        "added required parameter `n`",
    );
    expect(
        &gql("query a(id: uuid, n?: int) -> X"),
        &gql("query a(id: uuid) -> X"),
        Severity::Breaking,
        "removed parameter `n`",
    );
    expect(
        &gql("query a(n?: int) -> X"),
        &gql("query a(n: int) -> X"),
        Severity::Breaking,
        "now required",
    );
    expect(
        &gql("query a(n: int) -> X"),
        &gql("query a(n?: int) -> X"),
        Severity::Safe,
        "now optional",
    );
    // a query and a mutation may share a name, and are different fields
    expect(
        &gql("query a() -> X"),
        &gql("mutation a() -> X"),
        Severity::Breaking,
        "removed `a`",
    );
    let sock = |m: &str| format!("{base}socket s {{ {m} }}");
    expect(
        &sock("server a: X"),
        &sock("client a: X"),
        Severity::Breaking,
        "sending side",
    );
    expect(
        &sock("server a: X"),
        &sock("server a: Y"),
        Severity::Breaking,
        "payload changed",
    );
}

#[test]
fn client_socket_events_are_input_and_server_ones_output() {
    let s = |side: &str, f: &str| {
        format!("package p\ntype X {{ a: int {f} }}\nsocket s {{ {side} e: X }}")
    };
    expect(
        &s("client", ""),
        &s("client", "b: int"),
        Severity::Breaking,
        "added required field",
    );
    expect(
        &s("server", ""),
        &s("server", "b: int"),
        Severity::Safe,
        "added required field",
    );
}

#[test]
fn headers_of_an_event() {
    let e = |h: &str| format!("package p\ntype X {{ id: uuid }}\nevent a: X {{ {h} }}");
    expect(
        &e(""),
        &e("headers: { t: string }"),
        Severity::Safe,
        "added headers",
    );
    expect(
        &e("headers: { t: string }"),
        &e(""),
        Severity::Breaking,
        "removed headers",
    );
}

#[test]
fn no_change_means_no_changes() {
    let src = "package p\ntype X { id: uuid }\nevent a: X\nhttp { GET \"/a\" a() -> X }";
    let r = report(src, src);
    assert!(r.changes.is_empty());
    assert_eq!(r.required, Bump::None);
}

// ---- packages and versions ----

#[test]
fn packages_appear_and_disappear() {
    let a = ir("package a\ntype X { id: uuid }");
    let b = ir("package b\ntype Y { id: uuid }");
    let reports = compare(std::slice::from_ref(&a), &[a.clone(), b.clone()]);
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[1].package, "b");
    assert_eq!(worst(&reports[1]), Some(Severity::Safe));
    let reports = compare(&[a.clone(), b], &[a]);
    let gone = reports.iter().find(|r| r.package == "b").unwrap();
    assert_eq!(worst(gone), Some(Severity::Breaking));
    assert!(!gone.passes());
}

fn versioned(version: &str, fields: &str) -> String {
    format!("package p @version(\"{version}\")\ntype O {{ {fields} }}\nevent o.made: O")
}

#[test]
fn a_breaking_change_needs_a_major_bump() {
    let old = versioned("1.2.0", "id: uuid n: int");
    for (new_version, passes) in [
        ("1.2.0", false),
        ("1.2.1", false),
        ("1.3.0", false),
        ("2.0.0", true),
        ("3.1.0", true),
    ] {
        let r = report(&old, &versioned(new_version, "id: uuid"));
        assert_eq!(r.required, Bump::Major);
        assert_eq!(r.passes(), passes, "{new_version}");
        assert_eq!(r.version_is_enough(), passes, "{new_version}");
    }
}

#[test]
fn before_1_0_a_minor_bump_covers_breaking_changes() {
    let old = versioned("0.4.0", "id: uuid n: int");
    assert!(report(&old, &versioned("0.5.0", "id: uuid")).passes());
    assert!(!report(&old, &versioned("0.4.1", "id: uuid")).passes());
}

#[test]
fn a_breaking_change_without_versions_does_not_pass() {
    let r = report(
        "package p\ntype O { id: uuid n: int }\nevent o.made: O",
        "package p\ntype O { id: uuid }\nevent o.made: O",
    );
    assert!(!r.passes());
    let r = report(
        &versioned("1.0.0", "id: uuid n: int"),
        "package p\ntype O { id: uuid }\nevent o.made: O",
    );
    assert!(!r.passes());
}

#[test]
fn only_breaking_changes_fail_the_check() {
    // A new optional field with no version bump is only a warning.
    let r = report(
        &versioned("1.0.0", "id: uuid"),
        &versioned("1.0.0", "id: uuid n?: int"),
    );
    assert!(r.passes());
    assert_eq!(r.required, Bump::Minor);
    assert!(!r.version_is_enough());
    assert!(report(
        &versioned("1.0.0", "id: uuid"),
        &versioned("1.1.0", "id: uuid n?: int")
    )
    .version_is_enough());
    // Risky changes the same.
    let enum_old = "package p @version(\"1.0.0\")\nenum C { A }\ntype O { c: C }\nevent o.made: O";
    let enum_new =
        "package p @version(\"1.0.0\")\nenum C { A B }\ntype O { c: C }\nevent o.made: O";
    let r = report(enum_old, enum_new);
    assert!(r.passes() && !r.version_is_enough());
}

#[test]
fn the_required_bump_follows_the_worst_change() {
    assert_eq!(
        report(
            &versioned("1.0.0", "id: uuid"),
            &versioned("1.0.0", "id: uuid n?: int")
        )
        .required,
        Bump::Minor
    );
    assert_eq!(
        report(
            &versioned("1.0.0", "id: uuid n: int"),
            &versioned("1.0.0", "id: uuid")
        )
        .required,
        Bump::Major
    );
}

#[test]
fn documentation_only_changes_need_a_patch() {
    let old = "package p @version(\"1.0.0\")\n/// Old.\ntype O { id: uuid }\nevent o.made: O";
    let new = "package p @version(\"1.0.0\")\n/// New words.\ntype O { id: uuid }\nevent o.made: O";
    let r = report(old, new);
    assert!(r.changes.is_empty());
    assert_eq!(r.required, Bump::Patch);
}

#[test]
fn every_breaking_change_has_a_hint() {
    // The messages that fail a build should say what to do.
    let r = report(&both("id: uuid n: int"), &both("id: uuid"));
    for c in r
        .changes
        .iter()
        .filter(|c| c.severity == Severity::Breaking)
    {
        assert!(c.hint.is_some(), "{c:?}");
    }
}
