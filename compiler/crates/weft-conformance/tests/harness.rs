use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};
use weft_conformance::cases::{self, Case, Expect, Kind};
use weft_conformance::runner::{Process, Runner};
use weft_conformance::{compare, compile_contracts, run_runner, typeref, unknown_types};

fn conformance_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../conformance")
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

// ---- the type shorthand ----

#[test]
fn types_are_written_the_short_way() {
    assert_eq!(
        typeref::parse("int64").unwrap(),
        json!({ "kind": "primitive", "name": "int64" })
    );
    assert_eq!(
        typeref::parse("conformance.Order").unwrap(),
        json!({ "kind": "named", "package": "conformance", "name": "Order" })
    );
    assert_eq!(
        typeref::parse("conformance.Money[][]").unwrap(),
        json!({ "kind": "list", "item": { "kind": "list", "item": { "kind": "named", "package": "conformance", "name": "Money" } } })
    );
    assert_eq!(
        typeref::parse("map<conformance.Side, string[]>").unwrap(),
        json!({ "kind": "map", "key": { "kind": "named", "package": "conformance", "name": "Side" }, "value": { "kind": "list", "item": { "kind": "primitive", "name": "string" } } })
    );
    assert_eq!(
        typeref::parse("conformance.Pair<int, conformance.Page<string>>[]").unwrap()["item"]
            ["args"][1]["args"][0]["name"],
        "string"
    );
    for bad in [
        "",
        "Order",
        "map<int>",
        "int[",
        "conformance.Page<int",
        "int int",
        "map<int, int",
    ] {
        assert!(typeref::parse(bad).is_err(), "`{bad}` should not parse");
    }
}

// ---- the real cases are sound ----

#[test]
fn the_cases_load_and_every_type_they_use_exists() {
    let dir = conformance_dir();
    let irs = compile_contracts(&dir.join("contracts")).unwrap_or_else(|e| panic!("{e}"));
    let all = cases::load(&dir.join("cases")).unwrap();
    assert!(all.len() > 150, "only {} cases", all.len());
    assert_eq!(unknown_types(&all, &irs), Vec::<String>::new());
}

// ---- the comparison ----

fn case(expect: Expect) -> Case {
    Case {
        id: "c".into(),
        file: "f.yaml".into(),
        kind: Kind::Decode {
            ty: json!({}),
            wire: json!(null),
        },
        expect,
    }
}

#[test]
fn numbers_compare_by_value() {
    assert!(compare::same(&json!(1), &json!(1.0)));
    assert!(compare::same(
        &json!({ "a": [1.0, 2] }),
        &json!({ "a": [1, 2.0] })
    ));
    assert!(!compare::same(&json!(1), &json!(1.5)));
    assert!(!compare::same(&json!("1"), &json!(1)));
    assert!(!compare::same(
        &json!(9_007_199_254_740_993_i64),
        &json!(9_007_199_254_740_992_i64)
    ));
}

#[test]
fn a_valid_answer_must_match_the_canonical_form() {
    let c = case(Expect::Canonical(json!({ "a": 1 })));
    assert!(compare::check(&c, &json!({ "valid": true, "canonical": { "a": 1.0 } })).is_ok());
    assert!(
        compare::check(&c, &json!({ "valid": true, "canonical": { "a": 2 } }))
            .unwrap_err()
            .contains("expected canonical")
    );
    assert!(
        compare::check(&c, &json!({ "valid": false, "issues": [{ "path": "a" }] }))
            .unwrap_err()
            .contains("rejected with a")
    );
    assert!(compare::check(&c, &json!({ "canonical": {} }))
        .unwrap_err()
        .contains("no `valid`"));
}

#[test]
fn an_invalid_answer_must_have_exactly_the_issue_paths() {
    let c = case(Expect::Issues(vec!["b".into(), "a".into()]));
    let issues = |paths: &[&str]| json!({ "valid": false, "issues": paths.iter().map(|p| json!({ "path": p, "message": "m" })).collect::<Vec<_>>() });
    assert!(compare::check(&c, &issues(&["a", "b"])).is_ok());
    assert!(
        compare::check(&c, &issues(&["b", "a"])).is_ok(),
        "the order does not matter"
    );
    assert!(compare::check(&c, &issues(&["a"]))
        .unwrap_err()
        .contains("expected issues at [a, b], got [a]"));
    assert!(
        compare::check(&c, &issues(&["a", "b", "b"])).is_err(),
        "the same issue twice is not the same answer"
    );
    assert!(
        compare::check(&c, &json!({ "valid": true, "canonical": 1 }))
            .unwrap_err()
            .contains("accepted")
    );
}

// ---- driving a runner ----

type Wrong = Box<dyn Fn(&str, Value) -> Value>;

/// A runner that answers every case the way the case expects, unless told to be wrong.
struct Oracle {
    answers: HashMap<String, Expect>,
    wrong: Wrong,
    loaded: bool,
    protocol: &'static str,
}

fn oracle(cases: &[Case], wrong: impl Fn(&str, Value) -> Value + 'static) -> Oracle {
    let answers = cases
        .iter()
        .map(|c| (case_key(c), c.expect.clone()))
        .collect();
    Oracle {
        answers,
        wrong: Box::new(wrong),
        loaded: false,
        protocol: "0.1",
    }
}

fn case_key(c: &Case) -> String {
    match &c.kind {
        Kind::Decode { ty, wire } => format!("{ty}|{wire}"),
        Kind::Config { yaml, env } => format!("config|{yaml}|{env}"),
    }
}

impl Runner for Oracle {
    fn call(&mut self, request: Value) -> Result<Value, String> {
        match request["op"].as_str().unwrap() {
            "hello" => {
                Ok(json!({ "language": "oracle", "version": "1", "protocol": self.protocol }))
            }
            "load" => {
                assert!(request["ir"].as_array().is_some_and(|a| !a.is_empty()));
                self.loaded = true;
                Ok(json!({}))
            }
            op @ ("decode" | "config") => {
                assert!(self.loaded, "{op} before load");
                let k = if op == "decode" {
                    format!("{}|{}", request["type"], request["wire"])
                } else {
                    format!(
                        "config|{}|{}",
                        request["yaml"].as_str().unwrap(),
                        request["env"]
                    )
                };
                let expect = self.answers.get(&k).ok_or("no such case")?;
                let honest = match expect {
                    Expect::Canonical(c) => json!({ "valid": true, "canonical": c }),
                    Expect::Issues(paths) => {
                        json!({ "valid": false, "issues": paths.iter().map(|p| json!({ "path": p, "message": "m" })).collect::<Vec<_>>() })
                    }
                    Expect::Config(c) => json!({ "valid": true, "config": c }),
                    Expect::Problems(p) => json!({ "valid": false, "problems": p }),
                };
                Ok((self.wrong)(&k, honest))
            }
            other => Err(format!("unknown op {other}")),
        }
    }
}

fn setup() -> (Vec<Value>, Vec<Case>) {
    let dir = conformance_dir();
    (
        compile_contracts(&dir.join("contracts")).unwrap(),
        cases::load(&dir.join("cases")).unwrap(),
    )
}

#[test]
fn a_runner_that_agrees_with_every_case_passes_them_all() {
    let (irs, all) = setup();
    let mut o = oracle(&all, |_, a| a);
    let report = run_runner("oracle", &mut o, &irs, &all).unwrap();
    assert_eq!(report.results.len(), all.len());
    assert_eq!(
        report.failed(),
        0,
        "{:?}",
        report
            .results
            .iter()
            .filter(|r| !r.passed)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        (report.language.as_str(), report.version.as_str()),
        ("oracle", "1")
    );
}

#[test]
fn a_runner_that_gets_one_answer_wrong_fails_exactly_that_case() {
    let (irs, all) = setup();
    let victim = all
        .iter()
        .find(|c| c.id == "encoding.timestamp.positive-offset-becomes-utc")
        .unwrap();
    let target = case_key(victim);
    let mut o = oracle(&all, move |k, honest| {
        if *k == target {
            json!({ "valid": true, "canonical": "2026-11-23T10:00:00.000Z" }) // forgot to convert to UTC
        } else {
            honest
        }
    });
    let report = run_runner("oracle", &mut o, &irs, &all).unwrap();
    let failed: Vec<_> = report.results.iter().filter(|r| !r.passed).collect();
    assert_eq!(failed.len(), 1, "{failed:?}");
    assert_eq!(failed[0].id, victim.id);
    assert!(failed[0]
        .failure
        .as_deref()
        .unwrap()
        .contains("expected canonical \"2026-11-23T08:00:00.000Z\""));
}

#[test]
fn a_runner_that_accepts_what_it_should_reject_fails() {
    let (irs, all) = setup();
    let mut o = oracle(&all, |_, honest| {
        if honest["valid"] == json!(false) {
            json!({ "valid": true, "canonical": null })
        } else {
            honest
        }
    });
    let report = run_runner("oracle", &mut o, &irs, &all).unwrap();
    let invalid = all
        .iter()
        .filter(|c| matches!(c.expect, Expect::Issues(_) | Expect::Problems(_)))
        .count();
    assert!(invalid > 50);
    assert_eq!(report.failed(), invalid);
}

#[test]
fn a_runner_that_reports_the_wrong_paths_fails() {
    let (irs, all) = setup();
    let mut o = oracle(&all, |_, honest| {
        if honest["valid"] == json!(false) {
            json!({ "valid": false, "issues": [] })
        } else {
            honest
        }
    });
    let report = run_runner("oracle", &mut o, &irs, &all).unwrap();
    assert!(report.failed() > 50);
}

#[test]
fn a_runner_error_fails_the_case_and_the_run_goes_on() {
    struct Broken;
    impl Runner for Broken {
        fn call(&mut self, r: Value) -> Result<Value, String> {
            match r["op"].as_str().unwrap() {
                "hello" => Ok(json!({ "language": "broken", "version": "0", "protocol": "0.1" })),
                "load" => Ok(json!({})),
                _ => Err("boom".into()),
            }
        }
    }
    let (irs, all) = setup();
    let report = run_runner("broken", &mut Broken, &irs, &all).unwrap();
    assert_eq!(report.failed(), all.len());
    assert!(report.results[0]
        .failure
        .as_deref()
        .unwrap()
        .contains("boom"));
}

#[test]
fn a_runner_with_another_protocol_is_refused() {
    let (irs, all) = setup();
    let mut o = oracle(&all, |_, a| a);
    o.protocol = "9.9";
    assert!(run_runner("oracle", &mut o, &irs, &all)
        .unwrap_err()
        .contains("protocol 9.9"));
}

// ---- the files ----

fn write_cases(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = scratch(name);
    for (f, text) in files {
        fs::write(dir.join(f), text).unwrap();
    }
    dir
}

#[test]
fn a_case_file_with_a_mistake_says_which_case() {
    let bad =
        |name: &str, text: &str| cases::load(&write_cases(name, &[("a.yaml", text)])).unwrap_err();
    let both = bad(
        "both",
        "cases:\n  - { id: x, type: int, wire: 1, expect: { canonical: 1, issues: [] } }\n",
    );
    assert!(
        both.contains("case `x`") && both.contains("exactly one"),
        "{both}"
    );
    let neither = bad(
        "neither",
        "cases:\n  - { id: x, type: int, wire: 1, expect: {} }\n",
    );
    assert!(neither.contains("exactly one"), "{neither}");
    let typo = bad(
        "typo",
        "cases:\n  - { id: x, type: int, wire: 1, expect: { cannonical: 1 } }\n",
    );
    assert!(typo.contains("cannonical"), "{typo}");
    let ty = bad(
        "ty",
        "cases:\n  - { id: x, type: Order, wire: 1, expect: { canonical: 1 } }\n",
    );
    assert!(
        ty.contains("case `x`") && ty.contains("package.Name"),
        "{ty}"
    );
    let twice = cases::load(&write_cases(
        "twice",
        &[
            (
                "a.yaml",
                "cases:\n  - { id: x, type: int, wire: 1, expect: { canonical: 1 } }\n",
            ),
            (
                "b.yaml",
                "cases:\n  - { id: x, type: int, wire: 2, expect: { canonical: 2 } }\n",
            ),
        ],
    ))
    .unwrap_err();
    assert!(twice.contains("`x` is used twice"), "{twice}");
}

#[test]
fn a_case_that_names_a_missing_type_is_found_before_any_runner_runs() {
    let (irs, _) = setup();
    let dir = write_cases(
        "missing",
        &[(
            "a.yaml",
            "cases:\n  - { id: x, type: conformance.Nope, wire: 1, expect: { canonical: 1 } }\n",
        )],
    );
    let all = cases::load(&dir).unwrap();
    let problems = unknown_types(&all, &irs);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("`conformance.Nope`"));
}

// ---- a real process ----

/// Needs `node` and the built TypeScript runner (`make conformance` builds it). Without them this skips,
/// unless WEFT_REQUIRE_RUNNERS is set, as in CI.
#[test]
fn the_typescript_runner_passes_every_case() {
    let root = conformance_dir().join("..");
    let entry = root.join("sdks/typescript/packages/conformance-runner/dist/index.js");
    let node_ok = std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !node_ok || !entry.exists() {
        assert!(
            std::env::var("WEFT_REQUIRE_RUNNERS").is_err(),
            "WEFT_REQUIRE_RUNNERS is set but node or the built runner is missing"
        );
        eprintln!("skipping: node or the built TypeScript runner is missing");
        return;
    }
    let (irs, all) = setup();
    let mut p = Process::start(
        &["node".into(), entry.to_string_lossy().into_owned()],
        &root,
    )
    .unwrap();
    let report = run_runner("typescript", &mut p, &irs, &all).unwrap();
    assert_eq!(report.language, "typescript");
    assert_eq!(
        report.failed(),
        0,
        "{:?}",
        report
            .results
            .iter()
            .filter(|r| !r.passed)
            .collect::<Vec<_>>()
    );
    // It is a real process: a bad request is an error, not a crash.
    assert!(p
        .call(json!({ "op": "nope" }))
        .unwrap_err()
        .contains("unknown op"));
    assert!(p.call(json!({ "op": "hello" })).is_ok());
}

/// Needs `go`. Without it this skips, unless WEFT_REQUIRE_RUNNERS is set, as in CI.
#[test]
fn the_go_runner_passes_every_case() {
    let root = conformance_dir().join("..");
    let go_ok = std::process::Command::new("go")
        .arg("version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !go_ok {
        assert!(
            std::env::var("WEFT_REQUIRE_RUNNERS").is_err(),
            "WEFT_REQUIRE_RUNNERS is set but go is missing"
        );
        eprintln!("skipping: go is missing");
        return;
    }
    let (irs, all) = setup();
    let command: Vec<String> = ["go", "-C", "go", "run", "./cmd/weft-conformance-runner"]
        .map(String::from)
        .to_vec();
    let mut p = Process::start(&command, &root).unwrap();
    let report = run_runner("go", &mut p, &irs, &all).unwrap();
    assert_eq!(report.language, "go");
    assert_eq!(
        report.failed(),
        0,
        "{:?}",
        report
            .results
            .iter()
            .filter(|r| !r.passed)
            .collect::<Vec<_>>()
    );
}
