//! The `weft` binary. Commands are added by the tasks in phase P1.

mod exit;
mod files;
mod fmt;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// One interface for every way services talk.
#[derive(Parser)]
#[command(name = "weft", version, about, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Format .weft files
    Fmt(fmt::FmtArgs),
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Fmt(args) => fmt::run(args),
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
