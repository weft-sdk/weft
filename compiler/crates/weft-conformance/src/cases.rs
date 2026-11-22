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
    /// An operation's result, of which these fields (and only these) are checked.
    Result(Value),
    /// A pair case: values at paths in the steps' results, and paths that must hold the same value.
    Pair {
        /// A path such as `got.messages.0.payload` to the value it must contain.
        paths: serde_json::Map<String, Value>,
        /// Pairs of paths whose values must be the same.
        equal: Vec<(String, String)>,
    },
}

/// One step of a pair case: a request to one of the two runners.
#[derive(Clone, Debug)]
pub struct Step {
    /// The name the result is kept under, for later steps and the expectations.
    pub name: String,
    /// `$A` (the producer or caller) or `$B` (the consumer or server).
    pub on: String,
    pub op: String,
    pub args: Value,
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
    /// Any other operation (`messaging`, ...): the request is `op` and these arguments.
    Op { op: String, args: Value },
    /// Steps on two runners at once, for every ordered pair of runners, with real infrastructure.
    Pair {
        /// What it needs: `kafka`.
        needs: Vec<String>,
        steps: Vec<Step>,
    },
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
            Kind::Config { .. } | Kind::Op { .. } | Kind::Pair { .. } => None,
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
    op: Option<String>,
    args: Option<Value>,
    needs: Option<Vec<String>>,
    steps: Option<Vec<RawStep>>,
    expect: RawExpect,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStep {
    name: String,
    on: String,
    op: String,
    #[serde(default)]
    args: Option<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExpect {
    canonical: Option<Value>,
    issues: Option<Vec<String>>,
    config: Option<Value>,
    problems: Option<Vec<String>>,
    result: Option<Value>,
    paths: Option<serde_json::Map<String, Value>>,
    equal: Option<Vec<(String, String)>>,
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
        for mut raw in doc.cases {
            if !seen.insert(raw.id.clone()) {
                return Err(format!("{file}: the case id `{}` is used twice", raw.id));
            }
            let kind = match (raw.ty, raw.wire, raw.yaml) {
                (None, None, None) if raw.steps.is_some() => {
                    let steps: Vec<Step> = raw
                        .steps
                        .take()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|s| Step {
                            name: s.name,
                            on: s.on,
                            op: s.op,
                            args: s.args.unwrap_or_else(|| serde_json::json!({})),
                        })
                        .collect();
                    for st in &steps {
                        if st.on != "$A" && st.on != "$B" {
                            return Err(format!(
                                "{file}: case `{}`: step `{}` runs on `{}`, which is not `$A` or `$B`",
                                raw.id, st.name, st.on
                            ));
                        }
                    }
                    Kind::Pair {
                        needs: raw.needs.clone().unwrap_or_default(),
                        steps,
                    }
                }
                (None, None, None) if raw.op.is_some() => Kind::Op {
                    op: raw.op.clone().unwrap_or_default(),
                    args: raw.args.clone().unwrap_or_else(|| serde_json::json!({})),
                },
                (Some(ty), Some(wire), None) if raw.env.is_none() && raw.op.is_none() => Kind::Decode {
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
                    "{file}: case `{}`: a case has either `type` and `wire`, or `yaml` (and `env`), or `op` (and `args`), or `steps`",
                    raw.id
                ))
                }
            };
            let e = raw.expect;
            let decode = matches!(kind, Kind::Decode { .. });
            let config = matches!(kind, Kind::Config { .. });
            let op = matches!(kind, Kind::Op { .. });
            let pair = matches!(kind, Kind::Pair { .. });
            let plain = e.paths.is_none() && e.equal.is_none();
            let expect = match (e.canonical, e.issues, e.config, e.problems, e.result) {
                (Some(c), None, None, None, None) if decode && plain => Expect::Canonical(c),
                (None, Some(i), None, None, None) if decode && plain => Expect::Issues(i),
                (None, None, Some(c), None, None) if config && plain => Expect::Config(c),
                (None, None, None, Some(p), None) if config && plain => Expect::Problems(p),
                (None, None, None, None, Some(r)) if op && plain => Expect::Result(r),
                (None, None, None, None, None) if pair => Expect::Pair {
                    paths: e.paths.unwrap_or_default(),
                    equal: e.equal.unwrap_or_default(),
                },
                _ => {
                    return Err(format!(
                        "{file}: case `{}`: `expect` needs exactly one of `canonical` or `issues` (for `type`), `config` or `problems` (for `yaml`), `result` (for `op`), or `paths` and `equal` (for `steps`)",
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
