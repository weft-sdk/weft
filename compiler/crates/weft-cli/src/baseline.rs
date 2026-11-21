//! Where the older contracts come from, for `weft check`.

use std::path::{Path, PathBuf};
use std::process::Command;

use weft_check::Source;

use crate::files;
use crate::project;

/// The older contracts, and a short description of where they came from.
pub struct Baseline {
    pub sources: Vec<Source>,
    pub label: String,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// The contract files as they were at a git ref. The working tree is never touched.
///
/// `roots` are the project's contract folders. A folder that did not exist at the ref has no contracts there.
pub fn from_git(
    project_dir: &Path,
    roots: &[PathBuf],
    reference: Option<&str>,
) -> Result<Baseline, String> {
    let top = git(project_dir, &["rev-parse", "--show-toplevel"]).map_err(|e| {
        format!("the baseline comes from git, and this is not a git repository ({e})")
    })?;
    let top = PathBuf::from(top.trim())
        .canonicalize()
        .map_err(|e| e.to_string())?;

    let reference = match reference {
        Some(r) => r.to_string(),
        None => git(project_dir, &["describe", "--tags", "--abbrev=0"])
            .map(|t| t.trim().to_string())
            .map_err(|_| {
                "no baseline: the repository has no git tag. Tag a release, or pass `--baseline <ref>` or `--baseline-dir <path>`"
                    .to_string()
            })?,
    };
    git(
        &top,
        &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
    )
    .map_err(|_| format!("unknown git ref `{reference}`"))?;

    let mut sources = Vec::new();
    for root in roots {
        let root = root.canonicalize().unwrap_or_else(|_| root.clone());
        let rel = root
            .strip_prefix(&top)
            .map_err(|_| format!("{} is outside the git repository", root.display()))?;
        let rel = if rel.as_os_str().is_empty() {
            ".".to_string()
        } else {
            rel.to_string_lossy().replace('\\', "/")
        };
        let listing = git(
            &top,
            &["ls-tree", "-r", "--name-only", &reference, "--", &rel],
        )?;
        for path in listing.lines().filter(|l| l.ends_with(".weft")) {
            let text = git(&top, &["show", &format!("{reference}:{path}")])?;
            sources.push(Source::new(format!("{reference}:{path}"), text));
        }
    }
    Ok(Baseline {
        sources,
        label: format!("git {reference}"),
    })
}

/// The contract files in a folder of older contracts.
pub fn from_dir(dir: &Path, cwd: &Path) -> Result<Baseline, String> {
    let found = files::find_weft_files(&[dir.to_path_buf()])
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(Baseline {
        sources: project::read_sources(&found, cwd)?,
        label: dir.display().to_string(),
    })
}
