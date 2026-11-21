//! `weft fmt`

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;

use crate::{exit, files};

#[derive(Args)]
pub struct FmtArgs {
    /// Files or folders to format. Folders are searched for `.weft` files. Defaults to the current folder.
    paths: Vec<PathBuf>,
    /// Change nothing. List the files that would change and exit with 1 if there are any.
    #[arg(long)]
    check: bool,
}

pub fn run(args: FmtArgs) -> ExitCode {
    let paths = if args.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.paths
    };
    let found = match files::find_weft_files(&paths) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("weft: {e}");
            return exit::code(exit::USAGE);
        }
    };
    if found.is_empty() {
        eprintln!("weft: no .weft files found");
        return exit::code(exit::OK);
    }

    let mut changed = 0;
    let mut broken = false;
    for path in &found {
        let shown = path.display().to_string();
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("weft: {shown}: {e}");
                return exit::code(exit::USAGE);
            }
        };
        match weft_syntax::format(&shown, &source) {
            Err(errors) => {
                broken = true;
                for e in errors {
                    eprintln!("{:?}", miette::Report::new(e));
                }
            }
            Ok(formatted) if formatted == source => {}
            Ok(formatted) => {
                changed += 1;
                if args.check {
                    println!("would reformat {shown}");
                } else if let Err(e) = fs::write(path, formatted) {
                    eprintln!("weft: {shown}: {e}");
                    return exit::code(exit::USAGE);
                } else {
                    println!("formatted {shown}");
                }
            }
        }
    }

    if broken {
        exit::code(exit::CONTRACT)
    } else if args.check && changed > 0 {
        exit::code(exit::STALE)
    } else {
        exit::code(exit::OK)
    }
}
