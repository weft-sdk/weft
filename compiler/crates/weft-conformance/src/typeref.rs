//! The short way cases write a type: `int64`, `conformance.Order`, `conformance.Money[]`,
//! `map<string, int>`, `conformance.Page<conformance.Order>`. It becomes a type reference as in the IR.

use serde_json::{json, Value};

const PRIMITIVES: [&str; 12] = [
    "string",
    "bool",
    "int",
    "int32",
    "int64",
    "float",
    "float32",
    "float64",
    "decimal",
    "uuid",
    "timestamp",
    "bytes",
];

pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser {
        chars: text.chars().collect(),
        at: 0,
    };
    let value = p.ty()?;
    p.skip_space();
    if p.at != p.chars.len() {
        return Err(format!("unexpected `{}` in type `{text}`", p.rest()));
    }
    Ok(value)
}

struct Parser {
    chars: Vec<char>,
    at: usize,
}

impl Parser {
    fn skip_space(&mut self) {
        while self.chars.get(self.at).is_some_and(|c| c.is_whitespace()) {
            self.at += 1;
        }
    }

    fn rest(&self) -> String {
        self.chars[self.at..].iter().collect()
    }

    fn eat(&mut self, c: char) -> bool {
        self.skip_space();
        if self.chars.get(self.at) == Some(&c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn ident(&mut self) -> Result<String, String> {
        self.skip_space();
        let start = self.at;
        while self
            .chars
            .get(self.at)
            .is_some_and(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
        {
            self.at += 1;
        }
        if start == self.at {
            return Err(format!("expected a type, found `{}`", self.rest()));
        }
        Ok(self.chars[start..self.at].iter().collect())
    }

    fn ty(&mut self) -> Result<Value, String> {
        let name = self.ident()?;
        let mut value = if name == "map" {
            if !self.eat('<') {
                return Err("`map` needs `<key, value>`".into());
            }
            let key = self.ty()?;
            if !self.eat(',') {
                return Err("`map<key, value>` needs a comma".into());
            }
            let val = self.ty()?;
            if !self.eat('>') {
                return Err("`map<` is not closed".into());
            }
            json!({ "kind": "map", "key": key, "value": val })
        } else if PRIMITIVES.contains(&name.as_str()) {
            json!({ "kind": "primitive", "name": name })
        } else if let Some((package, type_name)) = name.split_once('.') {
            let mut args = Vec::new();
            if self.eat('<') {
                loop {
                    args.push(self.ty()?);
                    if self.eat('>') {
                        break;
                    }
                    if !self.eat(',') {
                        return Err("type arguments need `,` or `>`".into());
                    }
                }
            }
            let mut v = json!({ "kind": "named", "package": package, "name": type_name });
            if !args.is_empty() {
                v["args"] = Value::Array(args);
            }
            v
        } else {
            return Err(format!(
                "`{name}` is not a primitive, and a named type is written `package.Name`"
            ));
        };
        while self.eat('[') {
            if !self.eat(']') {
                return Err("`[` needs `]`".into());
            }
            value = json!({ "kind": "list", "item": value });
        }
        Ok(value)
    }
}
