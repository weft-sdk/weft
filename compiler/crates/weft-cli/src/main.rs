//! The `weft` binary. Commands are added by the tasks in phase P1.

use clap::Parser;

/// One interface for every way services talk.
#[derive(Parser)]
#[command(name = "weft", version, about)]
struct Cli {}

fn main() {
    Cli::parse();
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
