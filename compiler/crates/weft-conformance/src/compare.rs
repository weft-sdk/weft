//! Does a runner's answer match what the case expects?

use serde_json::{Number, Value};

use crate::cases::{Case, Expect};

/// JSON equality where `1` and `1.0` are the same number.
pub fn same(a: &Value, b: &Value) -> bool {
    normalise(a) == normalise(b)
}

fn normalise(v: &Value) -> Value {
    match v {
        Value::Number(n) => {
            if let Some(f) = n.as_f64().filter(|_| !n.is_i64() && !n.is_u64()) {
                if f.fract() == 0.0 && f.abs() < 9_007_199_254_740_992.0 {
                    return Value::Number(Number::from(f as i64));
                }
            }
            v.clone()
        }
        Value::Array(a) => Value::Array(a.iter().map(normalise).collect()),
        Value::Object(o) => {
            Value::Object(o.iter().map(|(k, x)| (k.clone(), normalise(x))).collect())
        }
        _ => v.clone(),
    }
}

/// `Ok(())` if the runner's result agrees with the case, else what differs, in words.
pub fn check(case: &Case, result: &Value) -> Result<(), String> {
    if let Expect::Result(want) = &case.expect {
        return subset(want, result);
    }
    let Some(valid) = result.get("valid").and_then(Value::as_bool) else {
        return Err(format!("the answer has no `valid`: {result}"));
    };
    match &case.expect {
        Expect::Canonical(want) if valid => {
            let got = result.get("canonical").unwrap_or(&Value::Null);
            if same(want, got) {
                Ok(())
            } else {
                Err(format!("expected canonical {want}, got {got}"))
            }
        }
        Expect::Canonical(want) => Err(format!(
            "expected it to be valid (canonical {want}), but it was rejected with {}",
            issue_paths(result).join(", ")
        )),
        Expect::Issues(want) if valid => Err(format!(
            "expected it to be rejected with {}, but it was accepted as {}",
            want.join(", "),
            result.get("canonical").unwrap_or(&Value::Null)
        )),
        Expect::Issues(want) => {
            let mut want = want.clone();
            let mut got = issue_paths(result);
            want.sort();
            got.sort();
            if want == got {
                Ok(())
            } else {
                Err(format!(
                    "expected issues at [{}], got [{}]",
                    want.join(", "),
                    got.join(", ")
                ))
            }
        }
        Expect::Config(want) if valid => {
            let got = result.get("config").unwrap_or(&Value::Null);
            if same(want, got) {
                Ok(())
            } else {
                Err(format!("expected config {want}, got {got}"))
            }
        }
        Expect::Config(want) => Err(format!(
            "expected the config {want}, but it was rejected with {:?}",
            problems(result)
        )),
        Expect::Result(_) => Ok(()),
        Expect::Problems(want) if valid => Err(format!(
            "expected it to be rejected ({} problem(s)), but it was accepted as {}",
            want.len(),
            result.get("config").unwrap_or(&Value::Null)
        )),
        Expect::Problems(want) => {
            let got = problems(result);
            let each_found = want
                .iter()
                .all(|needle| got.iter().any(|p| p.contains(needle.as_str())));
            if got.len() == want.len() && each_found {
                Ok(())
            } else {
                Err(format!(
                    "expected problems containing {want:?}, got {got:?}"
                ))
            }
        }
    }
}

/// `got` holds everything in `want`: objects field by field (extra fields in `got` are ignored, at any
/// depth), arrays element by element and of the same length, anything else as an equal value.
pub fn contains(want: &Value, got: &Value) -> bool {
    match (want, got) {
        (Value::Object(w), Value::Object(g)) => w
            .iter()
            .all(|(k, v)| g.get(k).is_some_and(|x| contains(v, x))),
        (Value::Array(w), Value::Array(g)) => {
            w.len() == g.len() && w.iter().zip(g).all(|(a, b)| contains(a, b))
        }
        _ => same(want, got),
    }
}

/// Every field of `want` is in `got`, as `contains` says.
fn subset(want: &Value, got: &Value) -> Result<(), String> {
    let Some(fields) = want.as_object() else {
        return Err("a `result` expectation must be an object".into());
    };
    let mut wrong = Vec::new();
    for (k, v) in fields {
        match got.get(k) {
            Some(g) if contains(v, g) => {}
            Some(g) => wrong.push(format!("{k}: expected {v}, got {g}")),
            None => wrong.push(format!(
                "{k}: expected {v}, but the answer has no such field"
            )),
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong.join("; "))
    }
}

fn problems(result: &Value) -> Vec<String> {
    result
        .get("problems")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| p.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn issue_paths(result: &Value) -> Vec<String> {
    result
        .get("issues")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|i| {
                    i.get("path")
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_default()
}
