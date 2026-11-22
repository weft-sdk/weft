//! The cases: language-neutral YAML files in `conformance/cases/`.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::typeref;

/// What a case expects of the runner's answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Expect {
    /// A `decode` that is valid, and this is the wire form after the round trip.
    Canonical(Value),
    /// A `decode` that is invalid, with exactly these issue paths (in any order).
    Issues(Vec<String>),
    /// A `config` that is valid, and reads as this.
    Config(Value),
    /// A `config` that is invalid: one problem for each of these, each containing the text.
    Problems(Vec<String>),
}

/// What a case asks the runner to do.
#[derive(Clone, Debug)]
pub enum Kind {
    /// `decode` a wire value as a type.
    Decode {
        /// The type, as a type reference.
        ty: Value,
        wire: Value,
    },
    /// `config`: read weft.yaml text with an environment.
    Config { yaml: String, env: Value },
}

#[derive(Clone, Debug)]
pub struct Case {
    pub id: String,
    pub file: String,
    pub kind: Kind,
    pub expect: Expect,
}

impl Case {
    /// The type reference of a `decode` case.
    pub fn ty(&self) -> Option<&Value> {
        match &self.kind {
            Kind::Decode { ty, .. } => Some(ty),
            Kind::Config { .. } => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileDoc {
    #[allow(dead_code)]
    description: Option<String>,
    cases: Vec<RawCase>,
}

/// A field that is there, even when its value is `null`: `Some(null)`, not `None`.
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCase {
    id: String,
    #[serde(rename = "type")]
    ty: Option<String>,
    #[serde(default, deserialize_with = "present")]
    wire: Option<Value>,
    yaml: Option<String>,
    env: Option<Value>,
    expect: RawExpect,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExpect {
    canonical: Option<Value>,
    issues: Option<Vec<String>>,
    config: Option<Value>,
    problems: Option<Vec<String>>,
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
            let kind = match (raw.ty, raw.wire, raw.yaml) {
                (Some(ty), Some(wire), None) if raw.env.is_none() => Kind::Decode {
                    ty: typeref::parse(&ty)
                        .map_err(|e| format!("{file}: case `{}`: {e}", raw.id))?,
                    wire,
                },
                (None, None, Some(yaml)) => Kind::Config {
                    yaml,
                    env: raw.env.unwrap_or_else(|| serde_json::json!({})),
                },
                _ => {
                    return Err(format!(
                    "{file}: case `{}`: a case has either `type` and `wire`, or `yaml` (and `env`)",
                    raw.id
                ))
                }
            };
            let e = raw.expect;
            let decode = matches!(kind, Kind::Decode { .. });
            let expect = match (e.canonical, e.issues, e.config, e.problems) {
                (Some(c), None, None, None) if decode => Expect::Canonical(c),
                (None, Some(i), None, None) if decode => Expect::Issues(i),
                (None, None, Some(c), None) if !decode => Expect::Config(c),
                (None, None, None, Some(p)) if !decode => Expect::Problems(p),
                _ => {
                    return Err(format!(
                        "{file}: case `{}`: `expect` needs exactly one of `canonical` or `issues` (for `type`), `config` or `problems` (for `yaml`)",
                        raw.id
                    ))
                }
            };
            cases.push(Case {
                id: raw.id,
                file: file.clone(),
                kind,
                expect,
            });
        }
    }
    Ok(cases)
}
