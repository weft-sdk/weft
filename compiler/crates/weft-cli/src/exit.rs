//! Exit codes, as listed in docs/cli.md.

use std::process::ExitCode;

pub const OK: u8 = 0;
/// `check` found breaking changes, `gen --check` found stale code, or `fmt --check` found unformatted files.
pub const STALE: u8 = 1;
/// A usage or configuration error.
pub const USAGE: u8 = 2;
/// A contract error: syntax or type.
pub const CONTRACT: u8 = 3;

pub fn code(c: u8) -> ExitCode {
    ExitCode::from(c)
}
