//! `weft gen`

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;
use weft_codegen::{Int64, Options, Transport};

use crate::exit;
use crate::project::{self, shown_path};

#[derive(Args)]
pub struct GenArgs {
    /// The language to generate code for. Defaults to `language:` in weft.yaml.
    #[arg(long)]
    lang: Option<String>,
    /// Where to write the generated file. Defaults to `codegen.out` in weft.yaml.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Write nothing. Exit with 1 if the generated file is missing or out of date (for CI).
    #[arg(long)]
    check: bool,
    /// Regenerate whenever a contract changes. Not available yet.
    #[arg(long)]
    watch: bool,
}

impl GenArgs {
    /// `weft gen` with every option left to `weft.yaml`.
    pub fn defaults() -> Self {
        GenArgs {
            lang: None,
            out: None,
            check: false,
            watch: false,
        }
    }
}

const DEFAULT_OUT: &str = "src/weft.gen.ts";
const DEFAULT_GO_OUT: &str = "weft.gen.go";

/// The Go package of the generated file: the name of the folder it goes in, as a Go identifier.
fn go_package_name(project: &std::path::Path, out: Option<&str>) -> String {
    let folder = out
        .and_then(|o| std::path::Path::new(o).parent())
        .and_then(|p| p.file_name())
        .or_else(|| project.file_name())
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let cleaned: String = folder
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if cleaned.is_empty() || cleaned.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        "weftgen".to_string()
    } else {
        cleaned
    }
}

fn usage(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("weft: {message}");
    exit::code(exit::USAGE)
}

pub fn run(args: GenArgs) -> ExitCode {
    if args.watch {
        return usage("`weft gen --watch` is not available yet");
    }
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => return usage(e),
    };

    let project = match project::load(&cwd) {
        Ok(p) => p,
        Err(e) => return usage(e),
    };
    let (cfg, dir) = (&project.config, &project.dir);

    // Language
    let lang = match args.lang.as_deref().or(cfg.language.as_deref()) {
        Some(l) => l.to_lowercase(),
        None => return usage("no language: set `language:` in weft.yaml or pass `--lang`"),
    };
    let is_go = match lang.as_str() {
        "typescript" | "ts" => false,
        "go" | "golang" => true,
        "javascript" | "js" | "python" | "java" | "kotlin" | "scala" | "csharp" | "fsharp"
        | "rust" | "ruby" | "php" | "elixir" | "swift" | "dart" => {
            return usage(format!(
                "`weft gen` does not generate {lang} yet. TypeScript and Go are built"
            ));
        }
        other => return usage(format!("unknown language `{other}`")),
    };

    // Options
    let int64 = match cfg.codegen.int64.as_deref() {
        None | Some("number") => Int64::Number,
        Some("bigint") => Int64::Bigint,
        Some(other) => {
            return usage(format!(
                "`codegen.int64` is `{other}`; use `number` or `bigint`"
            ))
        }
    };
    let options = Options {
        int64,
        service: cfg.service.clone(),
        transports: cfg
            .transports
            .iter()
            .map(|(name, t)| Transport {
                name: name.clone(),
                library: t.library.clone(),
            })
            .collect(),
        package: cfg
            .codegen
            .package
            .clone()
            .or_else(|| is_go.then(|| go_package_name(dir, cfg.codegen.out.as_deref()))),
        ..Options::default()
    };

    // Contracts
    let sources = match project.sources(&cwd) {
        Ok(s) => s,
        Err(e) => return usage(e),
    };

    // Check, then generate
    let irs = match weft_check::check(&sources) {
        Ok(irs) => irs,
        Err(errors) => {
            for e in errors {
                eprintln!("{e:?}");
            }
            return exit::code(exit::CONTRACT);
        }
    };
    let result = if is_go {
        weft_codegen::go(&irs, &options)
    } else {
        weft_codegen::typescript(&irs, &options)
    };
    let generated = match result {
        Ok(text) => text,
        Err(errors) => {
            for e in errors {
                eprintln!("weft: {e}");
            }
            return exit::code(exit::CONTRACT);
        }
    };

    // Write, or compare
    let out = match (&args.out, &cfg.codegen.out) {
        (Some(p), _) => cwd.join(p),
        (None, Some(p)) => dir.join(p),
        (None, None) => dir.join(if is_go { DEFAULT_GO_OUT } else { DEFAULT_OUT }),
    };
    let shown = shown_path(&out, &cwd);
    let existing = fs::read_to_string(&out).ok();
    if args.check {
        return match existing {
            Some(text) if text == generated => {
                println!("✓ {shown} is up to date");
                exit::code(exit::OK)
            }
            Some(_) => {
                eprintln!("✗ {shown} is out of date. Run `weft gen`");
                exit::code(exit::STALE)
            }
            None => {
                eprintln!("✗ {shown} does not exist. Run `weft gen`");
                exit::code(exit::STALE)
            }
        };
    }
    if existing.as_deref() == Some(generated.as_str()) {
        println!("✓ {shown} is up to date");
        return exit::code(exit::OK);
    }
    if let Some(parent) = out.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return usage(format!("{}: {e}", parent.display()));
        }
    }
    if let Err(e) = fs::write(&out, generated) {
        return usage(format!("{shown}: {e}"));
    }
    println!("✓ Wrote {shown}");
    exit::code(exit::OK)
}
