use std::fs;
use std::path::PathBuf;
use std::process::Command;

use weft_check::{check, Source};
use weft_codegen::{typescript, Int64, Options};
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
