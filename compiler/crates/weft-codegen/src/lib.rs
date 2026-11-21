//! Code generators. One module per language; each turns checked [`weft_ir::Ir`] into source text.
//!
//! Only TypeScript exists so far (task P1.6).

mod typescript;

use std::fmt;

pub use typescript::typescript;

/// How TypeScript represents the contract type `int` (64-bit). From `codegen.int64` in `weft.yaml`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Int64 {
    /// Safe up to 2^53.
    #[default]
    Number,
    /// The full 64-bit range.
    Bigint,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub int64: Int64,
    /// Written into the header of generated files, for example `weft 0.0.0`.
    pub generator: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            int64: Int64::Number,
            generator: concat!("weft ", env!("CARGO_PKG_VERSION")).to_string(),
        }
    }
}

/// Something in the contracts that cannot be written in the target language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodegenError {
    pub message: String,
    pub help: Option<String>,
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)?;
        if let Some(h) = &self.help {
            write!(f, " ({h})")?;
        }
        Ok(())
    }
}

impl std::error::Error for CodegenError {}
