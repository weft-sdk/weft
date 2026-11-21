//! Finding `.weft` files.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The `.weft` files at the given paths: a file is itself, a folder is searched recursively. Hidden folders,
/// `node_modules` and `target` are skipped. The result is sorted and has no duplicates.
pub fn find_weft_files(paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in paths {
        let meta = fs::metadata(p)?;
        if meta.is_dir() {
            walk(p, &mut out)?;
        } else {
            out.push(p.clone());
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.file_type()?.is_dir() {
            if !name.starts_with('.') && name != "node_modules" && name != "target" {
                walk(&path, out)?;
            }
        } else if path.extension().is_some_and(|e| e == "weft") {
            out.push(path);
        }
    }
    Ok(())
}
