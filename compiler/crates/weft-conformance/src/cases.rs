//! The cases: language-neutral YAML files in `conformance/cases/`.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::typeref;

/// What a case expects of a `decode`.
#[derive(Clone, Debug, PartialEq)]
pub enum Expect {
    /// Valid, and this is the wire form after the round trip.
    Canonical(Value),
    /// Invalid, with exactly these issue paths (in any order).
    Issues(Vec<String>),
}

#[derive(Clone, Debug)]
pub struct Case {
    pub id: String,
    pub file: String,
    /// The type, as a type reference.
    pub ty: Value,
    pub wire: Value,
    pub expect: Expect,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileDoc {
    #[allow(dead_code)]
    description: Option<String>,
    cases: Vec<RawCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCase {
    id: String,
    #[serde(rename = "type")]
    ty: String,
    wire: Value,
    expect: RawExpect,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExpect {
    canonical: Option<Value>,
    issues: Option<Vec<String>>,
}

/// Every `*.yaml` file in `dir`, in name order. Any mistake in a case is an error naming the case.
pub fn load(dir: &Path) -> Result<Vec<Case>, String> {
    let mut files: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .collect();
    files.sort();
    let mut cases = Vec::new();
    let mut seen = HashSet::new();
    for path in files {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(&path).map_err(|e| format!("{file}: {e}"))?;
        let doc: FileDoc = serde_yaml_ng::from_str(&text).map_err(|e| format!("{file}: {e}"))?;
        for raw in doc.cases {
            if !seen.insert(raw.id.clone()) {
                return Err(format!("{file}: the case id `{}` is used twice", raw.id));
            }
            let ty =
                typeref::parse(&raw.ty).map_err(|e| format!("{file}: case `{}`: {e}", raw.id))?;
            let expect = match (raw.expect.canonical, raw.expect.issues) {
                (Some(c), None) => Expect::Canonical(c),
                (None, Some(i)) => Expect::Issues(i),
                _ => {
                    return Err(format!(
                        "{file}: case `{}`: `expect` needs exactly one of `canonical` and `issues`",
                        raw.id
                    ))
                }
            };
            cases.push(Case {
                id: raw.id,
                file: file.clone(),
                ty,
                wire: raw.wire,
                expect,
            });
        }
    }
    Ok(cases)
}
