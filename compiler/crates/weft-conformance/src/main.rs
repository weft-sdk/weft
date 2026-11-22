//! `weft-conformance run`: runs the cases in `conformance/` against the runners in `conformance/runners.yaml`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde::Deserialize;
use weft_conformance::cases::Kind;
use weft_conformance::pairs::{run_pair, Infra, PairResult};
use weft_conformance::runner::{Process, Runner};
use weft_conformance::table;
use weft_conformance::{cases, compile_contracts, run_runner, unknown_types, RunnerReport};

#[derive(Parser)]
#[command(
    name = "weft-conformance",
    about = "Run the Weft conformance cases against each language's runner"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the cases
    Run {
        /// Only these runners (by name in runners.yaml). Default: all of them
        #[arg(long = "runner", value_name = "NAME")]
        only: Vec<String>,
        /// Only cases whose id contains this text
        #[arg(long)]
        filter: Option<String>,
        /// Write the results as JSON to this file
        #[arg(long, value_name = "FILE")]
        results: Option<PathBuf>,
        /// Write the compatibility table (Markdown) to this file
        #[arg(long, value_name = "FILE")]
        table: Option<PathBuf>,
        /// Fail, rather than skip, a case that needs infrastructure (Kafka) that is not there
        #[arg(long)]
        require_infra: bool,
        /// The conformance folder. Default: `conformance/` in this folder or one above
        #[arg(long, value_name = "DIR")]
        dir: Option<PathBuf>,
    },
}

#[derive(Deserialize)]
struct RunnersFile {
    runners: BTreeMap<String, RunnerDef>,
}

#[derive(Deserialize)]
struct RunnerDef {
    /// Relative to the repository root.
    command: Vec<String>,
}

fn find_dir(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let c = d.join("conformance");
        if c.join("runners.yaml").is_file() {
            return Some(c);
        }
        dir = d.parent();
    }
    None
}

fn main() -> ExitCode {
    let Cmd::Run {
        only,
        filter,
        results,
        table,
        require_infra,
        dir,
    } = Cli::parse().command;
    match run(
        &only,
        filter.as_deref(),
        results.as_deref(),
        table.as_deref(),
        require_infra,
        dir,
    ) {
        Ok(failed) => ExitCode::from(u8::from(failed)),
        Err(e) => {
            eprintln!("weft-conformance: {e}");
            ExitCode::from(2)
        }
    }
}

/// `Ok(true)` if any case failed.
fn run(
    only: &[String],
    filter: Option<&str>,
    results: Option<&Path>,
    table_file: Option<&Path>,
    require_infra: bool,
    dir: Option<PathBuf>,
) -> Result<bool, String> {
    let dir = match dir {
        Some(d) => d,
        None => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            find_dir(&cwd).ok_or("no conformance/runners.yaml found here or in a folder above")?
        }
    };
    let root = dir
        .parent()
        .ok_or("the conformance folder has no parent")?
        .to_path_buf();
    let runners: RunnersFile = serde_yaml_ng::from_str(
        &fs::read_to_string(dir.join("runners.yaml")).map_err(|e| format!("runners.yaml: {e}"))?,
    )
    .map_err(|e| format!("runners.yaml: {e}"))?;
    for name in only {
        if !runners.runners.contains_key(name) {
            return Err(format!("`{name}` is not in runners.yaml"));
        }
    }

    let irs = compile_contracts(&dir.join("contracts"))
        .map_err(|e| format!("the contracts do not compile:\n{e}"))?;
    let mut all = cases::load(&dir.join("cases"))?;
    let problems = unknown_types(&all, &irs);
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    if let Some(f) = filter {
        all.retain(|c| c.id.contains(f));
    }
    if all.is_empty() {
        return Err("no cases to run".into());
    }

    let mut reports: Vec<RunnerReport> = Vec::new();
    for (name, def) in &runners.runners {
        if !only.is_empty() && !only.contains(name) {
            continue;
        }
        let mut process =
            Process::start(&def.command, &root).map_err(|e| format!("runner `{name}`: {e}"))?;
        reports.push(
            run_runner(name, &mut process, &irs, &all)
                .map_err(|e| format!("runner `{name}`: {e}"))?,
        );
    }

    // Cases that need two languages and real infrastructure, for every ordered pair of runners.
    let infra = Infra {
        kafka: std::env::var("WEFT_TEST_KAFKA")
            .ok()
            .filter(|v| !v.is_empty()),
    };
    let chosen: Vec<(&String, &RunnerDef)> = runners
        .runners
        .iter()
        .filter(|(n, _)| only.is_empty() || only.contains(n))
        .collect();
    let mut pairs: Vec<PairResult> = Vec::new();
    let mut counter = 0u32;
    for case in all.iter().filter(|c| matches!(c.kind, Kind::Pair { .. })) {
        let Kind::Pair { needs, .. } = &case.kind else {
            continue;
        };
        let missing: Vec<&String> = needs.iter().filter(|n| !infra.has(n)).collect();
        for (a_name, a_def) in &chosen {
            for (b_name, b_def) in &chosen {
                let mut result = PairResult {
                    id: case.id.clone(),
                    caller: (*a_name).clone(),
                    server: (*b_name).clone(),
                    passed: false,
                    skipped: false,
                    failure: None,
                };
                if !missing.is_empty() {
                    if require_infra {
                        result.failure = Some(format!(
                            "needs {} (set WEFT_TEST_KAFKA)",
                            missing
                                .iter()
                                .map(|m| m.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    } else {
                        result.skipped = true;
                        result.passed = true;
                    }
                    pairs.push(result);
                    continue;
                }
                counter += 1;
                let run = format!(
                    "{}{counter}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_millis())
                );
                let outcome = start(a_def, &root, &irs).and_then(|mut a| {
                    let mut b = start(b_def, &root, &irs)?;
                    run_pair(case, &mut a, &mut b, &run, &infra)
                });
                match outcome {
                    Ok(()) => result.passed = true,
                    Err(e) => result.failure = Some(e),
                }
                pairs.push(result);
            }
        }
    }

    let mut failed = false;
    for r in &reports {
        let bad = r.failed();
        println!(
            "{} ({} {}): {} of {} cases passed",
            r.runner,
            r.language,
            r.version,
            r.results.len() - bad,
            r.results.len()
        );
        for f in r.results.iter().filter(|x| !x.passed) {
            failed = true;
            println!("  FAIL {}: {}", f.id, f.failure.as_deref().unwrap_or(""));
        }
    }
    if !pairs.is_empty() {
        let ran = pairs.iter().filter(|p| !p.skipped).count();
        let skipped = pairs.len() - ran;
        let bad = pairs.iter().filter(|p| !p.passed).count();
        println!(
            "pairs: {} of {ran} passed{}",
            ran - bad,
            if skipped > 0 {
                format!(", {skipped} skipped (no infrastructure)")
            } else {
                String::new()
            }
        );
        for p in pairs.iter().filter(|p| !p.passed) {
            failed = true;
            println!(
                "  FAIL {} [{} -> {}]: {}",
                p.id,
                p.caller,
                p.server,
                p.failure.as_deref().unwrap_or("")
            );
        }
    }
    if let Some(path) = results {
        let doc = serde_json::json!({ "cases": all.len(), "runners": reports, "pairs": pairs });
        fs::write(
            path,
            serde_json::to_string_pretty(&doc).expect("results serialise") + "\n",
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    if let Some(path) = table_file {
        fs::write(path, table::render(&reports, &pairs))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(failed)
}

/// Starts a runner and gives it the contracts.
fn start(def: &RunnerDef, root: &Path, irs: &[serde_json::Value]) -> Result<Process, String> {
    let mut p = Process::start(&def.command, root)?;
    p.call(serde_json::json!({ "op": "hello" }))?;
    p.call(serde_json::json!({ "op": "load", "ir": irs }))?;
    Ok(p)
}
