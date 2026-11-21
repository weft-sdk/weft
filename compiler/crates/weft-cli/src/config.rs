//! `weft.yaml`: the per-service config file. Only the keys a command needs are read; the rest are ignored here.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const FILE_NAME: &str = "weft.yaml";

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub language: Option<String>,
    #[serde(default)]
    pub contracts: Vec<ContractSource>,
    #[serde(default)]
    pub codegen: CodegenConfig,
}

/// One entry of `contracts:`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ContractSource {
    /// A local folder or file.
    Path(String),
    /// Packages from another repository. Read by `weft pull`, not by this build.
    Git {
        git: String,
        #[serde(default)]
        #[allow(dead_code)]
        packages: BTreeMap<String, String>,
    },
}

#[derive(Debug, Default, Deserialize)]
pub struct CodegenConfig {
    pub out: Option<String>,
    pub int64: Option<String>,
}

/// A config file and the folder that relative paths in it are relative to.
pub struct Loaded {
    pub config: Config,
    pub dir: PathBuf,
}

/// Find `weft.yaml` in `start` or the nearest parent folder, and read it.
pub fn load(start: &Path) -> Result<Loaded, String> {
    let start = start
        .canonicalize()
        .map_err(|e| format!("{}: {e}", start.display()))?;
    let mut dir = Some(start.as_path());
    while let Some(d) = dir {
        let candidate = d.join(FILE_NAME);
        if candidate.is_file() {
            let text = fs::read_to_string(&candidate)
                .map_err(|e| format!("{}: {e}", candidate.display()))?;
            let config: Config = if text.trim().is_empty() {
                Config::default()
            } else {
                serde_yaml_ng::from_str(&text)
                    .map_err(|e| format!("{}: {e}", candidate.display()))?
            };
            return Ok(Loaded {
                config,
                dir: d.to_path_buf(),
            });
        }
        dir = d.parent();
    }
    Err(format!(
        "no {FILE_NAME} found in {} or any folder above it. Run `weft init` to create one",
        start.display()
    ))
}
