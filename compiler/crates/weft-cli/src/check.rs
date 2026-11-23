//! `weft check`

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;
use weft_compat::{Bump, PackageReport, Severity};

use crate::{baseline, exit, project};

#[derive(Args)]
pub struct CheckArgs {
    /// Compare against the contracts at this git ref (a tag, branch or commit). Defaults to the latest tag.
    #[arg(long, value_name = "REF", conflicts_with = "baseline_dir")]
    baseline: Option<String>,
    /// Compare against the contracts in this folder instead of git.
    #[arg(long, value_name = "PATH")]
    baseline_dir: Option<PathBuf>,
    /// Write the result as JSON on standard output, and nothing else there. The exit codes are the same.
    #[arg(long)]
    json: bool,
}

fn usage(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("weft: {message}");
    exit::code(exit::USAGE)
}

pub fn run(args: CheckArgs) -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => return usage(e),
    };
    let project = match project::load(&cwd) {
        Ok(p) => p,
        Err(e) => return usage(e),
    };
    let current = match project.sources(&cwd) {
        Ok(s) => s,
        Err(e) => return usage(e),
    };
    let older = match &args.baseline_dir {
        Some(dir) => baseline::from_dir(&cwd.join(dir), &cwd),
        None => baseline::from_git(&project.dir, &project.roots, args.baseline.as_deref()),
    };
    let older = match older {
        Ok(b) => b,
        Err(e) => return usage(e),
    };

    let new_irs = match weft_check::check(&current) {
        Ok(i) => i,
        Err(errors) => {
            for e in errors {
                eprintln!("{e:?}");
            }
            return exit::code(exit::CONTRACT);
        }
    };
    let old_irs = if older.sources.is_empty() {
        Vec::new()
    } else {
        match weft_check::check(&older.sources) {
            Ok(i) => i,
            Err(errors) => {
                eprintln!(
                    "weft: the baseline ({}) has contract errors, so it cannot be compared:",
                    older.label
                );
                for e in errors {
                    eprintln!("{e:?}");
                }
                return exit::code(exit::CONTRACT);
            }
        }
    };

    let reports = weft_compat::compare(&old_irs, &new_irs);
    if args.json {
        let failed = reports.iter().any(|r| !r.passes());
        println!("{}", report_json(&older.label, &reports));
        return if failed {
            exit::code(exit::STALE)
        } else {
            exit::code(exit::OK)
        };
    }
    println!("Comparing with {}", older.label);
    let mut failed = false;
    for r in &reports {
        print_report(r);
        failed |= !r.passes();
    }
    if reports
        .iter()
        .all(|r| r.changes.is_empty() && r.required == Bump::None)
    {
        println!("✓ no changes");
    }
    if failed {
        println!("\n✗ breaking changes are not covered by the version");
        exit::code(exit::STALE)
    } else {
        exit::code(exit::OK)
    }
}

/// The result as JSON: every package compared, and every change with its severity, where, what and what to do.
fn report_json(baseline: &str, reports: &[PackageReport]) -> String {
    use serde_json::{json, Value};
    let packages: Vec<Value> = reports
        .iter()
        .map(|r| {
            let changes: Vec<Value> = [Severity::Breaking, Severity::Risky, Severity::Safe]
                .into_iter()
                .flat_map(|sev| r.changes.iter().filter(move |c| c.severity == sev))
                .map(|c| {
                    let mut v = json!({ "severity": c.severity.word(), "location": c.location, "message": c.message });
                    if let Some(h) = &c.hint {
                        v["hint"] = json!(h);
                    }
                    v
                })
                .collect();
            json!({
                "package": r.package,
                "oldVersion": r.old_version,
                "newVersion": r.new_version,
                "required": r.required.id(),
                "actual": r.actual.map(Bump::id),
                "passes": r.passes(),
                "counts": {
                    "breaking": r.count(Severity::Breaking),
                    "risky": r.count(Severity::Risky),
                    "safe": r.count(Severity::Safe),
                },
                "changes": changes,
            })
        })
        .collect();
    let doc = json!({
        "baseline": baseline,
        "passes": reports.iter().all(PackageReport::passes),
        "packages": packages,
    });
    serde_json::to_string_pretty(&doc).expect("the report serialises")
}

fn print_report(r: &PackageReport) {
    if r.changes.is_empty() && r.required == Bump::None {
        return;
    }
    let version = match (&r.old_version, &r.new_version) {
        (Some(o), Some(n)) if o == n => format!("version {n}"),
        (Some(o), Some(n)) => format!("version {o} → {n}"),
        (None, Some(n)) => format!("version {n}"),
        (Some(o), None) => format!("version {o} → none"),
        (None, None) => "no version".to_string(),
    };
    println!("\n{} ({version})", r.package);
    let width = r
        .changes
        .iter()
        .map(|c| c.location.len())
        .max()
        .unwrap_or(0);
    for sev in [Severity::Breaking, Severity::Risky, Severity::Safe] {
        for c in r.changes.iter().filter(|c| c.severity == sev) {
            println!("  {:<8}  {:<width$}  {}", sev.word(), c.location, c.message);
            if let Some(h) = &c.hint {
                println!("  {:<8}  {:<width$}  hint: {h}", "", "");
            }
        }
    }
    if r.changes.is_empty() {
        println!("  only documentation changed");
    }

    let (b, k, s) = (
        r.count(Severity::Breaking),
        r.count(Severity::Risky),
        r.count(Severity::Safe),
    );
    println!("  {b} breaking, {k} risky, {s} safe");
    if r.has_breaking() {
        if r.breaking_is_covered() {
            println!("  ✓ the version bump covers the breaking changes");
        } else if r.old_version.is_none() || r.new_version.is_none() {
            println!("  ✗ breaking changes need a version: add `@version(\"...\")` to the package and bump it");
        } else {
            let need = if r
                .old_version
                .as_deref()
                .is_some_and(|v| v.starts_with("0."))
            {
                "a minor (or major)"
            } else {
                "a major"
            };
            println!(
                "  ✗ breaking changes need {need} version bump; the version is {}",
                r.new_version.as_deref().unwrap_or("unset")
            );
        }
    } else if !r.version_is_enough() {
        println!(
            "  note: these changes call for {} version bump before release",
            r.required.word()
        );
    }
}
