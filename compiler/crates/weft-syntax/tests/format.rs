use weft_syntax::format;

const ORDERS: &str = include_str!("../../../../spec/examples/orders.weft");
const SINK: &str = include_str!("../../../../spec/examples/kitchen-sink.weft");

fn fmt(src: &str) -> String {
    format("t.weft", src).unwrap_or_else(|e| panic!("{e:#?}"))
}

fn assert_idempotent(src: &str) {
    let once = fmt(src);
    let twice = fmt(&once);
    assert_eq!(once, twice, "formatting is not idempotent");
}

#[test]
fn the_examples_are_already_formatted() {
    assert_eq!(
        fmt(ORDERS),
        ORDERS,
        "run `weft fmt` on spec/examples/orders.weft"
    );
    assert_eq!(
        fmt(SINK),
        SINK,
        "run `weft fmt` on spec/examples/kitchen-sink.weft"
    );
}

#[test]
fn idempotent_on_the_examples() {
    assert_idempotent(ORDERS);
    assert_idempotent(SINK);
}

const MESSY: &str = r#"// header
package   p   @version( "1.0.0" )
import   other


  /// Money.
type Money{amount:int,currency:Currency // the unit
 note ?: string   @deprecated}
enum Currency{EUR
GBP}
// before event
event   a.b : Money{headers:{x:string}   key:amount}  // after event
http /v1{
GET "/x/{id}" get(id:uuid)->Money|NotFound
POST "/long-route-name-for-wrapping" createSomethingWithAVeryLongName(body:Money,headers:{a:string,b:string})->Money|InvalidInput|Conflict
}
rpc S{m(Money)->stream Money  n(stream Money)->Money}



// dangling at the end
"#;

#[test]
fn messy_input_gets_the_canonical_style() {
    insta::assert_snapshot!(fmt(MESSY));
}

#[test]
fn idempotent_on_messy_input() {
    assert_idempotent(MESSY);
}

#[test]
fn comments_are_kept_in_place() {
    let out = fmt(MESSY);
    for c in [
        "// header",
        "// the unit",
        "// before event",
        "// after event",
        "// dangling at the end",
    ] {
        assert_eq!(out.matches(c).count(), 1, "{c}");
    }
    let lines: Vec<&str> = out.lines().collect();
    let before = lines.iter().position(|l| *l == "// before event").unwrap();
    assert!(lines[before + 1].starts_with("event a.b"));
    assert!(lines
        .iter()
        .any(|l| l.starts_with("event a.b") || l.starts_with("}") && l.contains("// after event")));
}

#[test]
fn blank_lines_collapse_to_one_and_declarations_are_separated() {
    let out = fmt("package p\ntype A { x: int }\ntype B { y: int }\n\n\n\ntype C { z: int }\n");
    assert_eq!(
        out,
        "package p\n\ntype A {\n  x: int\n}\n\ntype B {\n  y: int\n}\n\ntype C {\n  z: int\n}\n"
    );
}

#[test]
fn blank_lines_between_fields_are_kept_once() {
    let out = fmt("package p\ntype A {\n  a: int\n\n\n  b: int\n  c: int\n}\n");
    assert_eq!(
        out,
        "package p\n\ntype A {\n  a: int\n\n  b: int\n  c: int\n}\n"
    );
}

#[test]
fn enums_stay_on_one_line_unless_they_hold_comments_or_docs() {
    assert!(fmt("package p\nenum E { A B }").contains("enum E { A B }"));
    let multi = fmt("package p\nenum E {\n  A // first\n  B\n}");
    assert!(multi.contains("enum E {\n  A  // first\n  B\n}"), "{multi}");
    let docs = fmt("package p\nenum E {\n  /// a\n  A\n  B\n}");
    assert!(docs.contains("  /// a\n  A\n  B\n}"), "{docs}");
}

#[test]
fn empty_blocks_print_on_one_line() {
    let out = fmt("package p\ntype E {\n}\nhttp {\n}");
    assert!(out.contains("type E {}"));
    assert!(out.contains("http {}"));
}

#[test]
fn trailing_comments_in_a_block_are_aligned() {
    let out =
        fmt("package p\ntype A {\n  id: uuid // one\n  customerName: string // two\n  n: int\n}");
    assert!(
        out.contains("  id: uuid              // one\n  customerName: string  // two\n  n: int\n"),
        "{out}"
    );
}

#[test]
fn inline_types_with_comments_become_blocks() {
    let src = "package p\nevent a: {\n  x: int // the x\n  y: int\n}\n";
    let out = fmt(src);
    assert!(out.contains("// the x"));
    assert_idempotent(src);
}

#[test]
fn strings_are_escaped_again() {
    let out = fmt("package p\ntype A { x: string @pattern(\"a\\\\d\\\"\") }");
    assert!(out.contains("@pattern(\"a\\\\d\\\"\")"), "{out}");
}

#[test]
fn formatting_never_loses_a_comment_or_doc_line() {
    for src in [ORDERS, SINK, MESSY] {
        let out = fmt(src);
        let count = |s: &str| s.matches("//").count();
        assert_eq!(count(src), count(&out));
    }
}

#[test]
fn syntax_errors_are_reported_not_formatted() {
    assert!(format("t.weft", "package p\ntype A { x int }").is_err());
}
