//! Cases that need two languages talking to each other over real infrastructure. Each runs for every
//! ordered pair of runners: `$A` is the producer or caller, `$B` the consumer or server.

use serde::Serialize;
use serde_json::{json, Value};

use crate::cases::{Case, Expect, Kind, Step};
use crate::compare;
use crate::runner::Runner;

/// What the real infrastructure is. Cases name what they need; a case whose need is missing is skipped.
#[derive(Clone, Debug, Default)]
pub struct Infra {
    /// The Kafka brokers, such as `127.0.0.1:9092`.
    pub kafka: Option<String>,
}

impl Infra {
    pub fn has(&self, need: &str) -> bool {
        match need {
            "kafka" => self.kafka.is_some(),
            _ => false,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PairResult {
    pub id: String,
    pub caller: String,
    pub server: String,
    pub passed: bool,
    pub skipped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

/// A value at a dotted path: `got.messages.0.payload`. Numbers index arrays.
pub fn at<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut v = root;
    for part in path.split('.') {
        v = match v {
            Value::Array(a) => a.get(part.parse::<usize>().ok()?)?,
            Value::Object(o) => o.get(part)?,
            _ => return None,
        };
    }
    Some(v)
}

/// Replaces `$RUN` and `$BROKERS`, and `${path}` with the value at that path in the results so far. A string
/// that is exactly `${path}` takes the value's own type (a number stays a number).
pub fn substitute(value: &Value, run: &str, infra: &Infra, results: &Value) -> Value {
    match value {
        Value::String(s) => {
            if let Some(path) = s.strip_prefix("${").and_then(|r| r.strip_suffix('}')) {
                if !path.contains("${") {
                    if let Some(v) = at(results, path) {
                        return v.clone();
                    }
                }
            }
            let mut out = s
                .replace("$RUN", run)
                .replace("$BROKERS", infra.kafka.as_deref().unwrap_or(""));
            while let Some(start) = out.find("${") {
                let Some(len) = out[start..].find('}') else {
                    break;
                };
                let path = out[start + 2..start + len].to_string();
                let text = match at(results, &path) {
                    Some(Value::String(t)) => t.clone(),
                    Some(other) => other.to_string(),
                    None => String::new(),
                };
                out.replace_range(start..=start + len, &text);
            }
            Value::String(out)
        }
        Value::Array(a) => Value::Array(
            a.iter()
                .map(|x| substitute(x, run, infra, results))
                .collect(),
        ),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| (k.clone(), substitute(x, run, infra, results)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Runs one pair case with `a` as `$A` and `b` as `$B`. `Ok(())` if every step ran and every expectation holds.
pub fn run_pair(
    case: &Case,
    a: &mut dyn Runner,
    b: &mut dyn Runner,
    run: &str,
    infra: &Infra,
) -> Result<(), String> {
    let (Kind::Pair { steps, .. }, Expect::Pair { paths, equal }) = (&case.kind, &case.expect)
    else {
        return Err("not a pair case".into());
    };
    let mut results = json!({});
    for Step { name, on, op, args } in steps {
        let mut request = substitute(args, run, infra, &results);
        request["op"] = json!(op);
        let runner: &mut dyn Runner = if on == "$A" { a } else { b };
        let answer = runner
            .call(request)
            .map_err(|e| format!("step `{name}` ({op} on {on}): {e}"))?;
        results[name] = answer;
    }
    let mut wrong = Vec::new();
    for (path, want) in paths {
        let want = substitute(want, run, infra, &results);
        match at(&results, path) {
            Some(got) if compare::contains(&want, got) => {}
            Some(got) => wrong.push(format!("{path}: expected {want}, got {got}")),
            None => wrong.push(format!(
                "{path}: expected {want}, but there is no such value"
            )),
        }
    }
    for (x, y) in equal {
        match (at(&results, x), at(&results, y)) {
            (Some(vx), Some(vy)) if compare::same(vx, vy) => {}
            (Some(vx), Some(vy)) => {
                wrong.push(format!("{x} and {y} should be the same: {vx} and {vy}"))
            }
            _ => wrong.push(format!(
                "{x} and {y} should be the same, but one of them is missing"
            )),
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong.join("; "))
    }
}
