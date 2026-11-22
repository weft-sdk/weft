use std::fs;
use std::path::PathBuf;
use std::process::Command;

use weft_check::{check, Source};
use weft_codegen::{typescript, Int64, Options, Transport};
use weft_ir::Ir;

const ORDERS: &str = include_str!("../../../../spec/examples/orders.weft");
const SINK: &str = include_str!("../../../../spec/examples/kitchen-sink.weft");
const PAYMENTS: &str = r#"package payments @version("0.3.0")
import orders

/// What was charged.
type Charge {
  order: orders.Order
  amount: orders.Money
  /// Money by currency; the key is an enum from another package.
  byCurrency: map<orders.Currency, orders.Money>
}

type Money { cents: int64 }

event payment.captured: Charge
"#;

fn irs(files: &[(&str, &str)]) -> Vec<Ir> {
    let sources: Vec<Source> = files.iter().map(|(n, t)| Source::new(*n, *t)).collect();
    check(&sources).unwrap_or_else(|e| panic!("{e:?}"))
}

fn generate(files: &[(&str, &str)]) -> String {
    typescript(&irs(files), &Options::default()).unwrap_or_else(|e| panic!("{e:?}"))
}

#[test]
fn orders_output() {
    insta::assert_snapshot!(without_contracts(&generate(&[("orders.weft", ORDERS)])));
}

#[test]
fn kitchen_sink_types() {
    insta::assert_snapshot!(without_contracts(&generate(&[("sink.weft", SINK)])));
}

#[test]
fn two_packages_keep_their_own_names() {
    insta::assert_snapshot!(without_contracts(&generate(&[
        ("orders.weft", ORDERS),
        ("payments.weft", PAYMENTS)
    ])));
}

#[test]
fn output_is_deterministic() {
    let a = generate(&[("orders.weft", ORDERS), ("payments.weft", PAYMENTS)]);
    let b = generate(&[("payments.weft", PAYMENTS), ("orders.weft", ORDERS)]);
    assert_eq!(a, b);
}

#[test]
fn int64_can_be_bigint() {
    let ir = irs(&[(
        "o.weft",
        "package o\ntype A { a: int b: int64 c: int32 d?: int }",
    )]);
    let out = typescript(
        &ir,
        &Options {
            int64: Int64::Bigint,
            ..Options::default()
        },
    )
    .unwrap();
    assert!(out.contains("a: bigint;") && out.contains("b: bigint;") && out.contains("c: number;"));
    assert!(out.contains("d?: bigint | null;"));
}

#[test]
fn the_ir_is_embedded_and_matches() {
    let out = generate(&[("orders.weft", ORDERS)]);
    let start = out.find("export const contracts = {\n  orders: ").unwrap()
        + "export const contracts = {\n  orders: ".len();
    let end = out.rfind(",\n} as const;").unwrap();
    let json = out[start..end]
        .lines()
        .map(|l| l.strip_prefix("  ").unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n");
    let parsed = Ir::from_json(&json).expect("the embedded constant is the IR as JSON");
    assert_eq!(parsed, irs(&[("orders.weft", ORDERS)])[0]);
}

#[test]
fn names_typescript_cannot_hold_are_errors_that_say_which() {
    for (src, needle) in [
        (
            "package o\ntype string2 { a: int }\ntype Record { a: int }",
            "`Record`",
        ),
        ("package o\ntype Date { a: int }", "`Date`"),
        ("package o\ntype default { a: int }", "`default`"),
        ("package class\ntype A { a: int }", "package name `class`"),
        (
            "package o\ntype P<Record> { a: Record }",
            "type parameter `Record`",
        ),
    ] {
        let err = typescript(&irs(&[("o.weft", src)]), &Options::default()).unwrap_err();
        assert!(
            err.iter().any(|e| e.to_string().contains(needle)),
            "{needle}: {err:?}"
        );
    }
}

// ---- the typed `weft` object ----

fn transport(name: &str, library: &str) -> Transport {
    Transport {
        name: name.into(),
        library: Some(library.into()),
    }
}

fn with_facade(files: &[(&str, &str)], service: &str, transports: Vec<Transport>) -> String {
    typescript(
        &irs(files),
        &Options {
            service: Some(service.into()),
            transports,
            ..Options::default()
        },
    )
    .unwrap_or_else(|e| panic!("{e:?}"))
}

/// The part of the file from the facade's heading to its end.
fn facade_of(out: &str) -> &str {
    &out[out.find("// ---- The typed `weft` object ----").unwrap()..]
}

#[test]
fn without_transports_there_is_no_facade_and_no_import() {
    let out = generate(&[("orders.weft", ORDERS)]);
    assert!(!out.contains("@weft-sdk"));
    assert!(!out.contains("createWeft"));
}

#[test]
fn facade_for_http_and_kafka() {
    let out = with_facade(
        &[("orders.weft", ORDERS), ("payments.weft", PAYMENTS)],
        "orders",
        vec![transport("http", "express"), transport("kafka", "kafkajs")],
    );
    assert!(out.contains("from \"@weft-sdk/http-express\""));
    assert!(out.contains("from \"@weft-sdk/kafka-kafkajs\""));
    insta::assert_snapshot!(facade_of(&out));
}

#[test]
fn facade_only_for_what_has_a_package() {
    let out = with_facade(
        &[("orders.weft", ORDERS)],
        "orders",
        vec![
            transport("kafka", "kafkajs"),
            transport("gql", "apollo-server"),
            transport("http", "fastify"),
        ],
    );
    let facade = facade_of(&out);
    assert!(facade.contains("kafka: kafka as unknown as TypedMessaging<Events, Kafka>"));
    assert!(!facade.contains("TypedHttp<"), "{facade}");
    assert!(!out.contains("@weft-sdk/http-express"));
    assert!(facade.contains("Not available in the TypeScript package yet, so not in `weft`: gql (apollo-server), http (fastify)."));
}

#[test]
fn no_facade_when_no_transport_has_a_package() {
    let out = with_facade(
        &[("orders.weft", ORDERS)],
        "orders",
        vec![transport("gql", "apollo-server")],
    );
    assert!(!out.contains("createWeft"));
}

#[test]
fn an_unnamed_library_means_the_default() {
    let out = with_facade(
        &[("orders.weft", ORDERS)],
        "orders",
        vec![
            Transport {
                name: "http".into(),
                library: None,
            },
            Transport {
                name: "kafka".into(),
                library: None,
            },
        ],
    );
    assert!(out.contains("createWeft"));
}

#[test]
fn only_the_own_package_is_served_and_every_package_can_be_called() {
    let out = with_facade(
        &[("orders.weft", ORDERS), ("payments.weft", PAYMENTS)],
        "payments",
        vec![transport("http", "express")],
    );
    let routes = out
        .split("export type HttpRoutes = {")
        .nth(1)
        .unwrap()
        .split("\n};\n")
        .next()
        .unwrap();
    assert!(
        routes.trim().is_empty(),
        "payments has no routes of its own: {routes}"
    );
    assert!(
        out.contains("export type HttpServices = {\n  orders: {\n    "),
        "{out}"
    );
}

#[test]
fn the_same_event_in_two_packages_is_an_error() {
    let a = "package a\nevent thing.happened: { id: uuid }";
    let b = "package b\nevent thing.happened: { id: uuid }";
    let err = typescript(
        &irs(&[("a.weft", a), ("b.weft", b)]),
        &Options {
            service: Some("a".into()),
            transports: vec![transport("kafka", "kafkajs")],
            ..Options::default()
        },
    )
    .unwrap_err();
    assert!(
        err.iter().any(|e| e
            .to_string()
            .contains("`thing.happened` is declared in both")),
        "{err:?}"
    );
    // Without the facade the same contracts are fine.
    generate(&[("a.weft", a), ("b.weft", b)]);
}

#[test]
fn route_paths_are_keyed_by_full_path_and_by_short_path_unless_it_is_taken() {
    // one(): full /v1/t, short /t. two(): full /v1/v1/t, short /v1/t, which is one()'s full path.
    let src = "package s\ntype T { a: int }\nhttp /v1 {\n  GET \"/t\" one() -> T\n  GET \"/v1/t\" two() -> T\n  POST \"/u\" make(body: T) -> T\n}";
    let out = with_facade(&[("s.weft", src)], "s", vec![transport("http", "express")]);
    let routes = out
        .split("export type HttpRoutes = {")
        .nth(1)
        .unwrap()
        .split("\n};\n")
        .next()
        .unwrap();
    for key in [
        "\"GET /v1/t\"",
        "\"GET /t\"",
        "\"GET /v1/v1/t\"",
        "\"POST /v1/u\"",
        "\"POST /u\"",
    ] {
        assert_eq!(routes.matches(key).count(), 1, "{key} in {routes}");
    }
}

#[test]
fn the_committed_e2e_file_is_current() {
    // sdks/typescript/packages/e2e is generated by `make ts-fixtures`; this fails when it is stale.
    let committed = include_str!("../../../../sdks/typescript/packages/e2e/src/weft.gen.ts");
    let fresh = with_facade(
        &[("orders.weft", ORDERS)],
        "orders",
        vec![transport("http", "express"), transport("kafka", "kafkajs")],
    );
    assert_eq!(committed, fresh, "run `make ts-fixtures`");
}

/// Everything below the types, so snapshots stay readable. The IR has its own tests.
fn without_contracts(out: &str) -> String {
    out.split("\n/** The checked contracts")
        .next()
        .unwrap()
        .to_string()
}

// ---- the generated code type-checks ----

fn tsc() -> Option<String> {
    let candidate = std::env::var("TSC").unwrap_or_else(|_| "tsc".into());
    let ok = Command::new(&candidate)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if ok {
        Some(candidate)
    } else if std::env::var("WEFT_REQUIRE_TSC").is_ok_and(|v| !v.is_empty()) {
        panic!("tsc is required (WEFT_REQUIRE_TSC is set) but `{candidate}` was not found");
    } else {
        eprintln!("skipping: tsc not found. Set TSC, or WEFT_REQUIRE_TSC=1 to make this an error.");
        None
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Type-check `weft.gen.ts` and a consumer with the given flags. Returns tsc's output on failure.
fn typecheck(dir: &PathBuf, flags: &[&str]) -> Result<(), String> {
    let Some(tsc) = tsc() else { return Ok(()) };
    let o = Command::new(tsc)
        .current_dir(dir)
        .args([
            "--noEmit",
            "--strict",
            "--target",
            "es2022",
            "--module",
            "esnext",
            "--moduleResolution",
            "bundler",
        ])
        .args(flags)
        .args(["weft.gen.ts", "consumer.ts"])
        .output()
        .unwrap();
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stdout).into_owned())
    }
}

const CONSUMER_ORDERS: &str = r#"import { contracts, type orders, type Uuid } from "./weft.gen";

const id = "8a4e1a62-0000-4000-8000-000000000000" as Uuid;
const money: orders.Money = { amount: 1, currency: "EUR" };

const order: orders.Order = {
  id,
  customerId: "c1",
  items: [{ sku: "a", qty: 2 }],
  total: money,
  note: null,
  placedAt: new Date(),
};

// @ts-expect-error a currency that is not in the enum
const wrongCurrency: orders.Money = { amount: 1, currency: "JPY" };
// @ts-expect-error a plain string is not a uuid
const plainId: orders.Order = { ...order, id: "plain" };
// @ts-expect-error a required field is missing
const missing: orders.Money = { amount: 1 };
// @ts-expect-error amount is a number
const wrongType: orders.Money = { amount: "1", currency: "EUR" };

const placed: orders.Events["order.placed"] = order;
const cancelled: orders.Events["order.cancelled"] = { id, reason: "late" };
// @ts-expect-error an event that does not exist
type Nope = orders.Events["order.nope"];

const stock: orders.OutOfStock = { sku: "a", available: 0 };
const version: "orders" = contracts.orders.package;

export { wrongCurrency, plainId, missing, wrongType, placed, cancelled, stock, version };
export type { Nope };
"#;

#[test]
fn orders_type_checks_with_tsc_strict() {
    let dir = scratch("ts_orders");
    fs::write(
        dir.join("weft.gen.ts"),
        generate(&[("orders.weft", ORDERS)]),
    )
    .unwrap();
    fs::write(dir.join("consumer.ts"), CONSUMER_ORDERS).unwrap();
    typecheck(&dir, &[]).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn orders_output_is_erasable_syntax() {
    // The generated file must work where only type syntax may be erased (Node's type stripping, Deno, Bun).
    let dir = scratch("ts_erasable");
    fs::write(
        dir.join("weft.gen.ts"),
        generate(&[("orders.weft", ORDERS)]),
    )
    .unwrap();
    fs::write(dir.join("consumer.ts"), CONSUMER_ORDERS).unwrap();
    typecheck(
        &dir,
        &[
            "--erasableSyntaxOnly",
            "--isolatedModules",
            "--verbatimModuleSyntax",
        ],
    )
    .unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn kitchen_sink_type_checks_with_tsc_strict() {
    let dir = scratch("ts_sink");
    fs::write(dir.join("weft.gen.ts"), generate(&[("sink.weft", SINK)])).unwrap();
    fs::write(
        dir.join("consumer.ts"),
        r#"import type { kitchen, Uuid } from "./weft.gen";
const page: kitchen.Page<number> = { items: [1, 2], next: null };
const pair: kitchen.Pair<string, kitchen.Side> = { first: "a", second: "Left" };
const byKind: kitchen.Order["sides"] = { Left: 1 };
const tags: kitchen.Order["tags"] = { a: ["x"] };
const when: kitchen.Order["when"] = undefined;
const raw: kitchen.Order["nested"]["b"] = new Uint8Array();
const ev: kitchen.Events["page.viewed"] = { items: [], next: "n" };
const empty: kitchen.Empty = {};
// @ts-expect-error Side keys are Left | Right
const badKey: kitchen.Order["sides"] = { Up: 1 };
export { page, pair, byKind, tags, when, raw, ev, empty, badKey };
export type { Uuid };
"#,
    )
    .unwrap();
    typecheck(&dir, &[]).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn two_packages_type_check_together() {
    let dir = scratch("ts_two");
    fs::write(
        dir.join("weft.gen.ts"),
        generate(&[("orders.weft", ORDERS), ("payments.weft", PAYMENTS)]),
    )
    .unwrap();
    fs::write(
        dir.join("consumer.ts"),
        r#"import type { orders, payments, Uuid } from "./weft.gen";
declare const order: orders.Order;
const charge: payments.Charge = { order, amount: { amount: 1, currency: "EUR" }, byCurrency: { EUR: { amount: 1, currency: "EUR" } } };
// payments.Money and orders.Money are different types
const own: payments.Money = { cents: 5 };
// @ts-expect-error orders.Money has no `cents`
const mixed: orders.Money = { cents: 5 };
const ev: payments.Events["payment.captured"] = charge;
export { charge, own, mixed, ev };
export type { Uuid };
"#,
    )
    .unwrap();
    typecheck(&dir, &[]).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn bigint_output_type_checks() {
    let dir = scratch("ts_bigint");
    let out = typescript(
        &irs(&[("orders.weft", ORDERS)]),
        &Options {
            int64: Int64::Bigint,
            ..Options::default()
        },
    )
    .unwrap();
    fs::write(dir.join("weft.gen.ts"), out).unwrap();
    fs::write(
        dir.join("consumer.ts"),
        r#"import type { orders } from "./weft.gen";
const ok: orders.Money = { amount: 10n, currency: "EUR" };
// @ts-expect-error with int64: bigint a plain number is rejected
const bad: orders.Money = { amount: 10, currency: "EUR" };
export { ok, bad };
"#,
    )
    .unwrap();
    typecheck(&dir, &[]).unwrap_or_else(|e| panic!("{e}"));
}

// ---- Go ----

fn go_with_facade(files: &[(&str, &str)], service: &str) -> String {
    weft_codegen::go(
        &irs(files),
        &Options {
            service: Some(service.into()),
            package: Some("gen".into()),
            transports: vec![
                Transport {
                    name: "http".into(),
                    library: Some("nethttp".into()),
                },
                Transport {
                    name: "kafka".into(),
                    library: Some("franz".into()),
                },
            ],
            ..Options::default()
        },
    )
    .unwrap_or_else(|e| panic!("{e:?}"))
}

#[test]
fn go_types_for_the_orders_example() {
    let out = weft_codegen::go(&irs(&[("orders.weft", ORDERS)]), &Options::default()).unwrap();
    let types = out.split("\n// contractsJSON is").next().unwrap();
    insta::assert_snapshot!(types);
}

#[test]
fn go_without_transports_has_no_facade_and_no_adapter_imports() {
    let out = weft_codegen::go(&irs(&[("orders.weft", ORDERS)]), &Options::default()).unwrap();
    assert!(!out.contains("nethttp") && !out.contains("franz") && !out.contains("func New("));
    assert!(out.contains("package weftgen"));
    assert!(out.contains("func Contracts() *weft.Contracts"));
}

#[test]
fn go_names_initialisms_enums_optionals_and_generics_the_go_way() {
    let out = go_with_facade(&[("sink.weft", SINK)], "kitchen");
    for needle in [
        "CustomerID ",               // customerId
        "type KitchenSide string",   // an enum is a string type
        "KitchenSideLeft ",          // and its values are constants
        "type KitchenPage[T any] struct",
        "Items []T ",
        "*string ",                   // an optional scalar is a pointer
        "map[string][]string",       // a map is a map
        "map[KitchenSide]int64",
        "*float64",
        "`json:\"customer_id\"`",    // the wire name is the tag
        "type UUID string",
        "func (k *Kafka) PublishOrderPlaced(",
        "func (k *Kafka) OnPageViewed(handler func(ctx context.Context, payload KitchenPage[KitchenOrder]",
        "func (h *HTTP) HandleCreateOrder(",
        "func NewKitchenOutOfStock(",
        "func AsKitchenDeclined(",
        "// Deprecated: use Basket",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in\n{out}");
    }
}

#[test]
fn go_the_same_event_in_two_packages_is_an_error() {
    let a = "package a\nevent thing.happened: { id: uuid }";
    let b = "package b\nevent thing.happened: { id: uuid }";
    let err = weft_codegen::go(
        &irs(&[("a.weft", a), ("b.weft", b)]),
        &Options {
            service: Some("a".into()),
            ..Options::default()
        },
    )
    .unwrap_err();
    assert!(
        err.iter().any(|e| e
            .to_string()
            .contains("`thing.happened` is declared in both")),
        "{err:?}"
    );
}

#[test]
fn go_only_serves_the_routes_of_its_own_package() {
    let out = go_with_facade(
        &[("orders.weft", ORDERS), ("payments.weft", PAYMENTS)],
        "payments",
    );
    assert!(out.contains("func (h *HTTP) Orders() *OrdersClient"));
    assert!(out.contains("func (c *OrdersClient) CreateOrder("));
    assert!(
        !out.contains("func (h *HTTP) HandleCreateOrder("),
        "payments serves no routes"
    );
}

#[test]
fn go_the_committed_e2e_file_is_current() {
    // go/e2e is generated by `make go-fixtures`; this fails when it is stale.
    let committed = include_str!("../../../../go/e2e/weft.gen.go");
    let fresh = weft_codegen::go(
        &irs(&[("orders.weft", ORDERS)]),
        &Options {
            service: Some("orders".into()),
            package: Some("e2e".into()),
            transports: vec![
                Transport {
                    name: "http".into(),
                    library: Some("nethttp".into()),
                },
                Transport {
                    name: "kafka".into(),
                    library: Some("franz".into()),
                },
            ],
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(committed, fresh, "run `make go-fixtures`");
}

/// The folder of the Go module as `go.mod` wants it: forward slashes, and none of the `\\?\` prefix that
/// `canonicalize` puts on Windows paths, which Go refuses.
fn go_dir(repo: &std::path::Path) -> String {
    let path = repo.join("go").display().to_string();
    path.strip_prefix(r"\\?\")
        .unwrap_or(&path)
        .replace('\\', "/")
}

fn tool(name: &str, arg: &str) -> bool {
    Command::new(name)
        .arg(arg)
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Needs `go`. Without it this skips, unless WEFT_REQUIRE_GO is set, as in CI.
fn go_available() -> bool {
    if tool("go", "version") && tool("gofmt", "-h") {
        return true;
    }
    assert!(
        std::env::var("WEFT_REQUIRE_GO").is_err(),
        "WEFT_REQUIRE_GO is set but go is missing"
    );
    eprintln!("skipping: go is missing");
    false
}

#[test]
fn go_the_generated_code_is_gofmt_clean_and_compiles() {
    if !go_available() {
        return;
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("go_generated");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    fs::write(
        dir.join("go.mod"),
        format!(
            "module example.com/gen\n\ngo 1.26.0\n\nrequire github.com/weft-sdk/weft/go v0.0.0\n\nreplace github.com/weft-sdk/weft/go => {}\n",
            go_dir(&repo)
        ),
    )
    .unwrap();
    // The kitchen sink uses every construct: generics, maps with enum keys, nested inline types,
    // deprecations, declared errors, every route shape.
    fs::write(
        dir.join("weft.gen.go"),
        go_with_facade(&[("sink.weft", SINK)], "kitchen"),
    )
    .unwrap();

    let run = |args: &[&str]| {
        let out = Command::new(args[0])
            .args(&args[1..])
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{} failed:\n{}{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    assert_eq!(
        run(&["gofmt", "-l", "."]).trim(),
        "",
        "gofmt would change the generated file"
    );
    run(&["go", "mod", "tidy"]);
    run(&["go", "vet", "./..."]);
}
