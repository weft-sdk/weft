//! Finding a project's contracts: the config, the contract folders, and the files in them.

use std::fs;
use std::path::{Path, PathBuf};

use weft_check::Source;

use crate::config::{self, Config, ContractSource};
use crate::files;

pub struct Project {
    pub config: Config,
    /// The folder holding weft.yaml.
    pub dir: PathBuf,
    /// The folders (or files) named in `contracts:`, as absolute paths.
    pub roots: Vec<PathBuf>,
}

/// Load the config from `cwd` or a parent folder, and work out where the contracts are.
pub fn load(cwd: &Path) -> Result<Project, String> {
    let loaded = config::load(cwd)?;
    let mut roots = Vec::new();
    if loaded.config.contracts.is_empty() {
        roots.push(loaded.dir.join("contracts"));
    }
    for c in &loaded.config.contracts {
        match c {
            ContractSource::Path(p) => roots.push(loaded.dir.join(p)),
            ContractSource::Git { git, .. } => {
                return Err(format!(
                    "contracts from git (`{git}`) are not read by this build. Use a local folder for now"
                ));
            }
        }
    }
    Ok(Project {
        config: loaded.config,
        dir: loaded.dir,
        roots,
    })
}

impl Project {
    /// Every `.weft` file in the contract folders, read. Names in messages are relative to `cwd`.
    pub fn sources(&self, cwd: &Path) -> Result<Vec<Source>, String> {
        let found = files::find_weft_files(&self.roots).map_err(|e| format!("contracts: {e}"))?;
        if found.is_empty() {
            let shown: Vec<String> = self.roots.iter().map(|r| r.display().to_string()).collect();
            return Err(format!("no .weft files in {}", shown.join(", ")));
        }
        read_sources(&found, cwd)
    }
}

/// Read files into sources named by their path relative to `cwd`.
pub fn read_sources(paths: &[PathBuf], cwd: &Path) -> Result<Vec<Source>, String> {
    paths
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(Source::new(shown_path(path, cwd), text))
        })
        .collect()
}

/// A path relative to the current folder when it is inside it.
pub fn shown_path(path: &Path, cwd: &Path) -> String {
    let base = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    // The file may not exist yet (an output file), so resolve its folder instead.
    let canon = if path.exists() {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.parent()
            .and_then(|p| p.canonicalize().ok())
            .map_or_else(
                || path.to_path_buf(),
                |p| p.join(path.file_name().unwrap_or_default()),
            )
    };
    canon
        .strip_prefix(&base)
        .map_or_else(|_| canon.display().to_string(), |p| p.display().to_string())
}
