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
        Kind::Op { op, args } => format!("{op}|{args}"),
        Kind::Pair { steps, .. } => {
            format!("pair|{}", c.id).replace('|', "|") + &steps.len().to_string()
        }
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
            op => {
                assert!(self.loaded, "{op} before load");
                let k = match op {
                    "decode" => format!("{}|{}", request["type"], request["wire"]),
                    "config" => format!(
                        "config|{}|{}",
                        request["yaml"].as_str().unwrap(),
                        request["env"]
                    ),
                    other => {
                        let mut args = request.clone();
                        args.as_object_mut().unwrap().remove("op");
                        format!("{other}|{args}")
                    }
                };
                let expect = self.answers.get(&k).ok_or(format!("no such case: {k}"))?;
                let honest = match expect {
                    Expect::Canonical(c) => json!({ "valid": true, "canonical": c }),
                    Expect::Issues(paths) => {
                        json!({ "valid": false, "issues": paths.iter().map(|p| json!({ "path": p, "message": "m" })).collect::<Vec<_>>() })
                    }
                    Expect::Config(c) => json!({ "valid": true, "config": c }),
                    Expect::Problems(p) => json!({ "valid": false, "problems": p }),
                    Expect::Result(r) => r.clone(),
                    Expect::Pair { .. } => json!({}),
                };
                Ok((self.wrong)(&k, honest))
            }
        }
    }
}

/// How many cases one runner runs by itself: the pair cases need two runners and real infrastructure.
fn singles(all: &[Case]) -> usize {
    all.iter()
        .filter(|c| !matches!(c.kind, Kind::Pair { .. }))
        .count()
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
    assert_eq!(report.results.len(), singles(&all));
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
    assert_eq!(report.failed(), singles(&all));
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

#[test]
fn a_result_expectation_looks_inside_objects_and_arrays_but_not_at_extra_fields() {
    use weft_conformance::compare::contains;
    let got =
        json!({ "a": 1, "env": { "k": "v", "x": 2 }, "list": [{ "p": 1, "q": 2 }, { "p": 3 }] });
    assert!(contains(&json!({ "env": { "k": "v" } }), &got));
    assert!(contains(&json!({ "list": [{ "p": 1 }, { "p": 3 }] }), &got));
    assert!(
        !contains(&json!({ "list": [{ "p": 1 }] }), &got),
        "arrays have to be the same length"
    );
    assert!(!contains(&json!({ "env": { "k": "other" } }), &got));
    assert!(!contains(&json!({ "env": { "missing": 1 } }), &got));
    assert!(
        contains(&json!({ "a": 1.0 }), &got),
        "numbers compare by value"
    );
    assert!(!contains(&json!({ "a": null }), &got));
}

#[test]
fn no_two_cases_ask_the_same_question() {
    // The same input with two expectations is a contradiction, and the fake runner could not tell them apart.
    let (_, all) = setup();
    let mut seen = HashMap::new();
    for c in &all {
        if let Some(other) = seen.insert(case_key(c), c.id.clone()) {
            panic!("`{}` and `{other}` ask the same question", c.id);
        }
    }
}

// ---- pair cases ----

mod pair {
    use std::sync::{Arc, Mutex};

    use serde_json::{json, Value};
    use weft_conformance::cases::{self, Expect, Kind};
    use weft_conformance::pairs::{at, run_pair, substitute, Infra};
    use weft_conformance::runner::Runner;
    use weft_conformance::table;
    use weft_conformance::{CaseResult, RunnerReport};

    /// A pretend language: services keep what is published to them in a store shared by both runners, as a
    /// broker would. It answers like a runner that agrees with the case.
    #[derive(Clone)]
    struct Loopback {
        store: Arc<Mutex<Vec<Value>>>,
        trace: &'static str,
        break_it: bool,
    }

    impl Runner for Loopback {
        fn call(&mut self, request: Value) -> Result<Value, String> {
            match request["op"].as_str().unwrap() {
                "service.start" => Ok(json!({ "httpPort": 4321 })),
                "service.publish" => {
                    self.store.lock().unwrap().push(request["payload"].clone());
                    Ok(json!({ "ok": true, "traceId": self.trace }))
                }
                "service.received" => {
                    let store = self.store.lock().unwrap();
                    let messages: Vec<Value> = store
                        .iter()
                        .map(|p| json!({ "payload": if self.break_it { json!({}) } else { p.clone() }, "key": p["id"], "attempt": 1, "traceId": self.trace,
                            "envelope": { "type": "order.placed", "source": "orders", "datacontenttype": "application/json", "hasId": true, "hasTime": true } }))
                        .collect();
                    Ok(json!({ "count": messages.len(), "messages": messages }))
                }
                "service.stop" => Ok(json!({ "drained": true, "abandoned": 0 })),
                other => Err(format!("unknown op {other}")),
            }
        }
    }

    fn kafka_case() -> weft_conformance::cases::Case {
        let all = cases::load(&super::conformance_dir().join("cases")).unwrap();
        all.into_iter()
            .find(|c| c.id == "cross.kafka.an-event-reaches-the-other-language")
            .unwrap()
    }

    fn infra() -> Infra {
        Infra {
            kafka: Some("localhost:9092".into()),
        }
    }

    #[test]
    fn a_pair_case_runs_its_steps_on_both_runners_and_checks_what_came_out() {
        let store = Arc::new(Mutex::new(Vec::new()));
        let mut a = Loopback {
            store: store.clone(),
            trace: "t1",
            break_it: false,
        };
        let mut b = Loopback {
            store,
            trace: "t1",
            break_it: false,
        };
        run_pair(&kafka_case(), &mut a, &mut b, "r1", &infra()).unwrap();
    }

    #[test]
    fn a_receiver_that_changes_the_payload_fails_the_case_with_the_path() {
        let store = Arc::new(Mutex::new(Vec::new()));
        let mut a = Loopback {
            store: store.clone(),
            trace: "t1",
            break_it: false,
        };
        let mut b = Loopback {
            store,
            trace: "t1",
            break_it: true,
        };
        let err = run_pair(&kafka_case(), &mut a, &mut b, "r1", &infra()).unwrap_err();
        assert!(err.contains("got.messages.0.payload"), "{err}");
    }

    #[test]
    fn a_trace_that_does_not_continue_fails_the_case() {
        let store = Arc::new(Mutex::new(Vec::new()));
        let mut a = Loopback {
            store: store.clone(),
            trace: "one",
            break_it: false,
        };
        let mut b = Loopback {
            store,
            trace: "another",
            break_it: false,
        };
        let err = run_pair(&kafka_case(), &mut a, &mut b, "r1", &infra()).unwrap_err();
        assert!(
            err.contains("publish.traceId and got.messages.0.traceId should be the same"),
            "{err}"
        );
    }

    #[test]
    fn a_step_that_the_runner_cannot_do_is_named() {
        struct Never;
        impl Runner for Never {
            fn call(&mut self, _: Value) -> Result<Value, String> {
                Err("not today".into())
            }
        }
        let err = run_pair(&kafka_case(), &mut Never, &mut Never, "r1", &infra()).unwrap_err();
        assert!(
            err.contains("step `consumer` (service.start on $B): not today"),
            "{err}"
        );
    }

    #[test]
    fn paths_and_substitution() {
        let results = json!({ "server": { "httpPort": 4321, "list": [{ "a": "x" }] } });
        assert_eq!(at(&results, "server.list.0.a"), Some(&json!("x")));
        assert_eq!(at(&results, "server.list.1"), None);
        assert_eq!(at(&results, "server.httpPort.deeper"), None);
        let i = infra();
        assert_eq!(
            substitute(&json!("${server.httpPort}"), "r9", &i, &results),
            json!(4321),
            "a whole reference keeps its type"
        );
        assert_eq!(
            substitute(
                &json!("http://127.0.0.1:${server.httpPort}/x"),
                "r9",
                &i,
                &results
            ),
            json!("http://127.0.0.1:4321/x")
        );
        assert_eq!(
            substitute(
                &json!({ "t": "placed-$RUN", "b": "$BROKERS", "l": ["$RUN"] }),
                "r9",
                &i,
                &results
            ),
            json!({ "t": "placed-r9", "b": "localhost:9092", "l": ["r9"] })
        );
    }

    #[test]
    fn every_real_pair_case_names_runners_and_steps_that_make_sense() {
        let all = cases::load(&super::conformance_dir().join("cases")).unwrap();
        let pairs: Vec<_> = all
            .iter()
            .filter(|c| matches!(c.kind, Kind::Pair { .. }))
            .collect();
        assert!(pairs.len() >= 10, "only {} pair cases", pairs.len());
        for c in pairs {
            let Kind::Pair { steps, needs } = &c.kind else {
                unreachable!()
            };
            let Expect::Pair { paths, equal } = &c.expect else {
                panic!("{}: not a pair expectation", c.id)
            };
            assert!(
                !paths.is_empty() || !equal.is_empty(),
                "{} expects nothing",
                c.id
            );
            let names: Vec<&str> = steps.iter().map(|s| s.name.as_str()).collect();
            let mut seen = std::collections::HashSet::new();
            for n in &names {
                assert!(seen.insert(*n), "{}: step `{n}` twice", c.id);
            }
            // Every path an expectation reads starts with the name of a step.
            for p in paths.keys().chain(equal.iter().flat_map(|(a, b)| [a, b])) {
                let first = p.split('.').next().unwrap();
                assert!(
                    names.contains(&first),
                    "{}: `{p}` does not start with a step name {names:?}",
                    c.id
                );
            }
            for n in needs {
                assert_eq!(n, "kafka", "{}: unknown need", c.id);
            }
        }
    }

    #[test]
    fn the_table_says_only_what_the_run_showed() {
        let report = |runner: &str, passed: bool| RunnerReport {
            runner: runner.into(),
            language: runner.into(),
            version: "0.0.0".into(),
            results: vec![
                CaseResult {
                    id: "encoding.int.zero".into(),
                    passed: true,
                    failure: None,
                },
                CaseResult {
                    id: "encoding.int.max".into(),
                    passed,
                    failure: None,
                },
                CaseResult {
                    id: "http.serve.x".into(),
                    passed: true,
                    failure: None,
                },
            ],
        };
        let reports = vec![report("go", true), report("typescript", false)];
        let pair =
            |c: &str, s: &str, passed: bool, skipped: bool| weft_conformance::pairs::PairResult {
                id: "cross.x".into(),
                caller: c.into(),
                server: s.into(),
                passed,
                skipped,
                failure: None,
            };
        let text = table::render(&reports, &[]);
        assert!(
            text.contains("| encoding | 2 / 2 | 1 / 2 |")
                && text.contains("| http | 1 / 1 | 1 / 1 |"),
            "{text}"
        );
        assert!(
            text.contains("Not run: it needs real infrastructure"),
            "no pairs, no claim: {text}"
        );

        let text = table::render(
            &reports,
            &[
                pair("go", "typescript", true, false),
                pair("go", "typescript", false, false),
                pair("typescript", "go", true, false),
                pair("go", "go", true, true),
            ],
        );
        assert!(text.contains("| go | 0 / 0 | 1 / 2 |"), "{text}");
        assert!(text.contains("| typescript | 1 / 1 | 0 / 0 |"), "{text}");
        assert!(text.contains("1 pair case(s) were skipped"), "{text}");
    }
}
