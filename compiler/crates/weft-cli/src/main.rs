//! The `weft` binary. Commands are added by the tasks in phase P1.

mod baseline;
mod check;
mod config;
mod exit;
mod files;
mod fmt;
mod generate;
mod project;

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
    /// Detect breaking contract changes
    Check(check::CheckArgs),
    /// Format .weft files
    Fmt(fmt::FmtArgs),
    /// Generate typed code from contracts
    Gen(generate::GenArgs),
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Check(args) => check::run(args),
        Command::Fmt(args) => fmt::run(args),
        Command::Gen(args) => generate::run(args),
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
