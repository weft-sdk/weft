//! `weft init`

use std::fs;
use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use inquire::{InquireError, MultiSelect, Select, Text};
use weft_init::{
    detect, example_contract, sanitize_service_name, transport, transports, validate_service_name,
    weft_yaml, Choices, Language, Transport,
};

use crate::{exit, generate};

/// Cancelled with Ctrl-C, by convention.
const CANCELLED: u8 = 130;

/// Each transport flag takes a library, or nothing for the default: `--http express` or `--http`.
macro_rules! transport_flags {
    ($($field:ident),*) => {
        #[derive(Args)]
        struct TransportFlags {
            $(
                #[arg(
                    long,
                    num_args = 0..=1,
                    default_missing_value = "",
                    value_name = "LIBRARY",
                    help = concat!("Use the `", stringify!($field), "` transport, with this library (default: the first)")
                )]
                $field: Option<String>,
            )*
        }
        impl TransportFlags {
            /// The transports given on the command line, each with its library (empty for the default).
            fn given(&self) -> Vec<(&'static str, String)> {
                let mut out = Vec::new();
                $( if let Some(l) = &self.$field { out.push((stringify!($field), l.clone())); } )*
                out
            }
        }
    };
}

transport_flags!(http, gql, grpc, rpc, ws, sse, kafka, mq, nats, redis, cloud, mqtt, pulsar);

#[derive(Args)]
pub struct InitArgs {
    /// Ask nothing: use the flags, and defaults for the rest.
    #[arg(long, short = 'y')]
    yes: bool,
    /// The service name (also its contract package). Defaults to the folder name.
    #[arg(long)]
    name: Option<String>,
    /// The language. Defaults to the one detected.
    #[arg(long)]
    lang: Option<String>,
    /// Where contracts live.
    #[arg(long, value_name = "PATH")]
    contracts: Option<String>,
    #[command(flatten)]
    transports: TransportFlags,
}

fn usage(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("weft: {message}");
    exit::code(exit::USAGE)
}

pub fn run(args: InitArgs) -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => return usage(e),
    };
    match init(&args, &cwd) {
        Ok(()) => generate::run(generate::GenArgs::defaults()),
        Err(Failure::Usage(m)) => usage(m),
        Err(Failure::Cancelled) => {
            eprintln!("weft: cancelled");
            exit::code(CANCELLED)
        }
    }
}

enum Failure {
    Usage(String),
    Cancelled,
}

impl From<String> for Failure {
    fn from(m: String) -> Self {
        Failure::Usage(m)
    }
}

impl From<InquireError> for Failure {
    fn from(e: InquireError) -> Self {
        match e {
            InquireError::OperationCanceled | InquireError::OperationInterrupted => {
                Failure::Cancelled
            }
            other => Failure::Usage(format!("could not ask: {other}")),
        }
    }
}

fn init(args: &InitArgs, cwd: &Path) -> Result<(), Failure> {
    let detected = detect(cwd);
    let language = match args.lang.as_deref() {
        Some(name) => Language::parse(name).ok_or_else(|| {
            format!("`weft init` does not set up {name} yet. TypeScript is the first language")
        })?,
        None => detected.language.unwrap_or(Language::TypeScript),
    };
    match (detected.language, detected.package_manager) {
        (Some(l), Some(pm)) => println!("Detected: {l} ({pm})"),
        (Some(l), None) => println!("Detected: {l}"),
        _ => println!("No project detected here: setting up {language}"),
    }

    if cwd.join(crate::config::FILE_NAME).exists() {
        return Err(Failure::Usage(
            "weft.yaml already exists here. `weft init` never overwrites; edit it, or remove it to start again".into(),
        ));
    }

    let interactive = !args.yes;
    if interactive && !std::io::stdin().is_terminal() {
        return Err(Failure::Usage(
            "`weft init` asks questions and needs a terminal. Pass `--yes` (with flags such as `--http express`) to run without one".into(),
        ));
    }

    let default_name = sanitize_service_name(
        &cwd.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    let service = match &args.name {
        Some(n) => n.clone(),
        None if interactive => Text::new("Service name")
            .with_default(&default_name)
            .with_validator(|s: &str| {
                Ok(match validate_service_name(s) {
                    Ok(()) => inquire::validator::Validation::Valid,
                    Err(e) => inquire::validator::Validation::Invalid(e.into()),
                })
            })
            .prompt()?,
        None => default_name,
    };
    validate_service_name(&service)?;

    let given = args.transports.given();
    let chosen: Vec<(&'static Transport, String)> = if !given.is_empty() {
        given
            .into_iter()
            .map(|(id, lib)| {
                (
                    transport(language, id).expect("every flag names a transport"),
                    lib,
                )
            })
            .collect()
    } else if interactive {
        ask_transports(language)?
    } else {
        vec![(
            transport(language, "http").expect("http exists"),
            String::new(),
        )]
    };

    let mut picked = Vec::new();
    for (t, requested) in chosen {
        let library = if requested.is_empty() {
            if interactive && t.libraries.len() > 1 {
                ask_library(t)?
            } else {
                t.libraries
                    .first()
                    .map(|l| l.id.to_string())
                    .unwrap_or_default()
            }
        } else if t.libraries.iter().any(|l| l.id == requested) {
            requested
        } else {
            let valid: Vec<&str> = t.libraries.iter().map(|l| l.id).collect();
            return Err(Failure::Usage(if valid.is_empty() {
                format!(
                    "`{}` has no library to choose: it runs on the HTTP library",
                    t.id
                )
            } else {
                format!(
                    "`{requested}` is not a {} library. Choose one of: {}",
                    t.label,
                    valid.join(", ")
                )
            }));
        };
        picked.push((t.id.to_string(), library));
    }

    let contracts = match &args.contracts {
        Some(c) => c.clone(),
        None if interactive => Text::new("Where do contracts live?")
            .with_default("./contracts")
            .prompt()?,
        None => "./contracts".to_string(),
    };
    if contracts.ends_with(".weft") {
        return Err(Failure::Usage(
            "`--contracts` is a folder, not a file".into(),
        ));
    }

    let choices = Choices {
        language,
        service,
        contracts,
        out: "src/weft.gen.ts".to_string(),
        transports: picked,
    };

    // Check everything in memory before writing anything.
    let contract = example_contract(&choices);
    let irs = weft_check::check(&[weft_check::Source::new("example.weft", contract.clone())])
        .map_err(|e| {
            format!("the example contract does not check (this is a bug in weft init): {e:?}")
        })?;
    if let Err(errors) = weft_codegen::typescript(&irs, &weft_codegen::Options::default()) {
        let m: Vec<String> = errors.iter().map(ToString::to_string).collect();
        return Err(Failure::Usage(format!(
            "the service name `{}` cannot be used: {}",
            choices.service,
            m.join("; ")
        )));
    }

    let contract_path = cwd
        .join(&choices.contracts)
        .join(format!("{}.weft", choices.service));
    for path in [cwd.join(&choices.out), contract_path.clone()] {
        if path.exists() {
            return Err(Failure::Usage(format!(
                "{} already exists. `weft init` never overwrites",
                crate::project::shown_path(&path, cwd)
            )));
        }
    }

    let write = |path: &Path, text: &str| -> Result<(), Failure> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(())
    };
    write(&cwd.join(crate::config::FILE_NAME), &weft_yaml(&choices))?;
    println!("✓ Wrote weft.yaml");
    write(&contract_path, &contract)?;
    println!(
        "✓ Wrote {} (example)",
        crate::project::shown_path(&contract_path, cwd)
    );
    Ok(())
}

fn ask_transports(language: Language) -> Result<Vec<(&'static Transport, String)>, Failure> {
    let all = transports(language);
    let labels: Vec<String> = all.iter().map(|t| t.label.to_string()).collect();
    let picked = MultiSelect::new("Which communication do you use?", labels)
        .with_default(&[0])
        .with_validator(|chosen: &[inquire::list_option::ListOption<&String>]| {
            Ok(if chosen.is_empty() {
                inquire::validator::Validation::Invalid("choose at least one".into())
            } else {
                inquire::validator::Validation::Valid
            })
        })
        .raw_prompt()?;
    Ok(picked
        .into_iter()
        .map(|p| (&all[p.index], String::new()))
        .collect())
}

fn ask_library(t: &Transport) -> Result<String, Failure> {
    let labels: Vec<String> = t.libraries.iter().map(|l| l.label.to_string()).collect();
    let picked = Select::new(&format!("{} library?", t.label), labels).raw_prompt()?;
    Ok(t.libraries[picked.index].id.to_string())
}
