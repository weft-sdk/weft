//! The conformance harness: compiles the contracts to IR, hands them to each language's runner, and checks
//! every case ([docs/07-conformance.md](../../../docs/07-conformance.md), [spec/conformance-protocol.md](../../../spec/conformance-protocol.md)).

pub mod cases;
pub mod compare;
pub mod pairs;
pub mod runner;
pub mod table;
pub mod typeref;

use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::{json, Value};
use weft_check::Source;

use crate::cases::{Case, Kind};
use crate::runner::Runner;

/// Compiles every `.weft` file in `dir` into IR documents.
pub fn compile_contracts(dir: &Path) -> Result<Vec<Value>, String> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "weft"))
        .collect();
    files.sort();
    let mut sources = Vec::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(&path).map_err(|e| format!("{name}: {e}"))?;
        sources.push(Source::new(name, text));
    }
    let irs = weft_check::check(&sources).map_err(|errors| {
        errors
            .iter()
            .map(|e| format!("{e:?}"))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    Ok(irs
        .iter()
        .map(|ir| serde_json::to_value(ir).expect("the IR always serialises"))
        .collect())
}

/// A named type that is not in the contracts: a case that could never pass.
pub fn unknown_types(cases: &[Case], irs: &[Value]) -> Vec<String> {
    fn walk(ty: &Value, out: &mut Vec<(String, String)>) {
        match ty.get("kind").and_then(Value::as_str) {
            Some("named") => {
                if let (Some(p), Some(n)) = (ty["package"].as_str(), ty["name"].as_str()) {
                    out.push((p.to_string(), n.to_string()));
                }
                for a in ty["args"].as_array().into_iter().flatten() {
                    walk(a, out);
                }
            }
            Some("list") => walk(&ty["item"], out),
            Some("map") => {
                walk(&ty["key"], out);
                walk(&ty["value"], out);
            }
            _ => {}
        }
    }
    let mut problems = Vec::new();
    for case in cases {
        let mut named = Vec::new();
        if let Some(ty) = case.ty() {
            walk(ty, &mut named);
        }
        for (package, name) in named {
            let found = irs.iter().any(|ir| {
                ir["package"] == json!(package)
                    && ir["types"]
                        .as_array()
                        .is_some_and(|t| t.iter().any(|t| t["name"] == json!(name)))
            });
            if !found {
                problems.push(format!(
                    "{}: case `{}` uses `{package}.{name}`, which the contracts do not declare",
                    case.file, case.id
                ));
            }
        }
    }
    problems
}

#[derive(Debug, Serialize)]
pub struct CaseResult {
    pub id: String,
    pub passed: bool,
    /// Why it failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunnerReport {
    pub runner: String,
    pub language: String,
    pub version: String,
    pub results: Vec<CaseResult>,
}

impl RunnerReport {
    pub fn failed(&self) -> usize {
        self.results.iter().filter(|r| !r.passed).count()
    }
}

/// Runs every case against one runner.
pub fn run_runner(
    name: &str,
    runner: &mut dyn Runner,
    irs: &[Value],
    cases: &[Case],
) -> Result<RunnerReport, String> {
    let hello = runner.call(json!({ "op": "hello" }))?;
    let text = |k: &str| {
        hello
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string()
    };
    if text("protocol") != "0.1" {
        return Err(format!(
            "the runner speaks protocol {}, this harness speaks 0.1",
            text("protocol")
        ));
    }
    runner.call(json!({ "op": "load", "ir": irs }))?;
    let mut results = Vec::new();
    for case in cases {
        if matches!(case.kind, Kind::Pair { .. }) {
            continue; // needs two runners and real infrastructure: see `pairs`
        }
        let request = match &case.kind {
            Kind::Decode { ty, wire } => json!({ "op": "decode", "type": ty, "wire": wire }),
            Kind::Config { yaml, env } => json!({ "op": "config", "yaml": yaml, "env": env }),
            Kind::Op { op, args } => {
                let mut request = args.clone();
                request["op"] = json!(op);
                request
            }
            Kind::Pair { .. } => unreachable!("skipped above"),
        };
        let answer = runner.call(request);
        let outcome = match answer {
            Ok(result) => compare::check(case, &result),
            Err(e) => Err(format!("the runner failed: {e}")),
        };
        results.push(CaseResult {
            id: case.id.clone(),
            passed: outcome.is_ok(),
            failure: outcome.err(),
        });
    }
    Ok(RunnerReport {
        runner: name.to_string(),
        language: text("language"),
        version: text("version"),
        results,
    })
}
