//! Checking of `@annotations`: which ones exist, where they may go, and what their arguments mean.

use std::collections::HashSet;

use regex::Regex;
use weft_ir::{Format, Primitive, Rule, TypeRef};
use weft_syntax::ast::{Annotation, AnnotationArg};

use crate::error::suggest;
use crate::lower::Lower;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Package,
    Type,
    Enum,
    Variant,
    Error,
    Field,
    Event,
    /// A route, method, GraphQL field or socket event.
    Operation,
}

impl Target {
    fn allowed(self) -> &'static [&'static str] {
        match self {
            Target::Package => &["version"],
            Target::Type | Target::Enum | Target::Variant | Target::Event | Target::Operation => {
                &["deprecated"]
            }
            Target::Error => &["deprecated", "status"],
            Target::Field => &[
                "deprecated",
                "wire",
                "min",
                "max",
                "length",
                "pattern",
                "format",
            ],
        }
    }

    fn describe(self) -> &'static str {
        match self {
            Target::Package => "a package",
            Target::Type => "a type",
            Target::Enum => "an enum",
            Target::Variant => "an enum value",
            Target::Error => "an error",
            Target::Field => "a field",
            Target::Event => "an event",
            Target::Operation => "an operation",
        }
    }
}

#[derive(Default)]
pub(crate) struct Anns {
    pub deprecated: Option<String>,
    pub wire: Option<String>,
    pub status: Option<u16>,
    pub version: Option<String>,
    pub rules: Vec<Rule>,
}

const ALL: &[&str] = &[
    "version",
    "deprecated",
    "wire",
    "status",
    "min",
    "max",
    "length",
    "pattern",
    "format",
];

#[derive(PartialEq)]
enum Class {
    Int,
    Float,
    Str,
    Bytes,
    List,
    Map,
    Other,
}

fn class(ty: Option<&TypeRef>) -> Option<Class> {
    Some(match ty? {
        TypeRef::Primitive { name } => match name {
            Primitive::Int | Primitive::Int32 | Primitive::Int64 => Class::Int,
            Primitive::Float | Primitive::Float32 | Primitive::Float64 | Primitive::Decimal => {
                Class::Float
            }
            Primitive::String => Class::Str,
            Primitive::Bytes => Class::Bytes,
            _ => Class::Other,
        },
        TypeRef::List { .. } => Class::List,
        TypeRef::Map { .. } => Class::Map,
        _ => Class::Other,
    })
}

fn is_semver(v: &str) -> bool {
    let re = Regex::new(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$").unwrap();
    re.is_match(v)
}

impl Lower<'_> {
    /// Check annotations on something of kind `target`. `ty` is the field's type when known, so validation rules
    /// can be matched to it. Reports problems and returns what was understood.
    pub fn annotations(
        &mut self,
        anns: &[Annotation],
        target: Target,
        ty: Option<&TypeRef>,
    ) -> Anns {
        let mut out = Anns::default();
        let mut seen = HashSet::new();
        let cls = class(ty);
        let shown = ty.map(crate::resolve::show);

        for ann in anns {
            let name = ann.name.name.as_str();
            if !ALL.contains(&name) {
                let help = match suggest(name, ALL.iter().copied()) {
                    Some(s) => format!("did you mean `@{s}`?"),
                    None => format!(
                        "annotations on {} are: {}",
                        target.describe(),
                        list(target.allowed())
                    ),
                };
                self.error(
                    &ann.span,
                    format!("unknown annotation `@{name}`"),
                    "not an annotation",
                    Some(help),
                );
                continue;
            }
            if !target.allowed().contains(&name) {
                self.error(
                    &ann.span,
                    format!("`@{name}` cannot be used on {}", target.describe()),
                    "not allowed here",
                    Some(format!(
                        "annotations on {} are: {}",
                        target.describe(),
                        list(target.allowed())
                    )),
                );
                continue;
            }
            if !seen.insert(name) {
                self.error(
                    &ann.span,
                    format!("`@{name}` is used twice"),
                    "duplicate",
                    None,
                );
                continue;
            }
            let args = ann.args.as_slice();
            let bad_args = |this: &mut Self, expected: &str| {
                this.error(
                    &ann.span,
                    format!("`@{name}` takes {expected}"),
                    "wrong arguments",
                    None,
                );
            };
            match (name, args) {
                ("deprecated", []) => out.deprecated = Some(String::new()),
                ("deprecated", [AnnotationArg::Str(m)]) => out.deprecated = Some(m.clone()),
                ("deprecated", _) => {
                    bad_args(self, "an optional message: `@deprecated(\"use x\")`")
                }

                ("version", [AnnotationArg::Str(v)]) => {
                    if is_semver(v) {
                        out.version = Some(v.clone());
                    } else {
                        self.error(
                            &ann.span,
                            format!("`{v}` is not a semantic version"),
                            "expected MAJOR.MINOR.PATCH",
                            Some("for example `@version(\"1.0.0\")`".into()),
                        );
                    }
                }
                ("version", _) => bad_args(self, "a version string: `@version(\"1.0.0\")`"),

                ("wire", [AnnotationArg::Str(w)]) if !w.is_empty() => out.wire = Some(w.clone()),
                ("wire", _) => bad_args(self, "a name: `@wire(\"total_cents\")`"),

                ("status", [AnnotationArg::Num(n)]) => match n.parse::<u16>() {
                    Ok(code) if (400..=599).contains(&code) => out.status = Some(code),
                    _ => self.error(
                        &ann.span,
                        format!("`{n}` is not an HTTP error status"),
                        "expected 400 to 599",
                        None,
                    ),
                },
                ("status", _) => bad_args(self, "a status code: `@status(402)`"),

                ("min" | "max", [AnnotationArg::Num(n)]) => match cls {
                    Some(Class::Int) if n.parse::<i64>().is_err() => self.error(
                        &ann.span,
                        format!("`@{name}` on an integer needs a whole number, not `{n}`"),
                        "not an integer",
                        None,
                    ),
                    Some(Class::Int | Class::Float) | None => {
                        out.rules.push(if name == "min" {
                            Rule::Min { value: n.clone() }
                        } else {
                            Rule::Max { value: n.clone() }
                        });
                    }
                    Some(_) => self.error(
                        &ann.span,
                        format!("`@{name}` applies to numbers"),
                        format!("this field is {}", shown.clone().unwrap_or_default()),
                        None,
                    ),
                },
                ("min" | "max", _) => bad_args(self, "a number: `@min(1)`"),

                ("length", [AnnotationArg::Num(lo), AnnotationArg::Num(hi)]) => {
                    match (lo.parse::<u64>(), hi.parse::<u64>()) {
                        (Ok(min), Ok(max)) if min <= max => match cls {
                            Some(Class::Str | Class::Bytes | Class::List | Class::Map) | None => {
                                out.rules.push(Rule::Length { min, max });
                            }
                            Some(_) => self.error(
                                &ann.span,
                                "`@length` applies to strings, lists and maps",
                                format!("this field is {}", shown.clone().unwrap_or_default()),
                                None,
                            ),
                        },
                        (Ok(_), Ok(_)) => self.error(
                            &ann.span,
                            format!("`@length` minimum {lo} is above its maximum {hi}"),
                            "minimum must not exceed maximum",
                            None,
                        ),
                        _ => self.error(
                            &ann.span,
                            "`@length` needs two whole numbers of zero or more",
                            "not valid lengths",
                            None,
                        ),
                    }
                }
                ("length", _) => bad_args(self, "a minimum and a maximum: `@length(1, 200)`"),

                ("pattern", [AnnotationArg::Str(re)]) => {
                    if !matches!(cls, Some(Class::Str) | None) {
                        self.error(
                            &ann.span,
                            "`@pattern` applies to strings",
                            format!("this field is {}", shown.clone().unwrap_or_default()),
                            None,
                        );
                    } else if let Err(e) = Regex::new(re) {
                        self.error(
                            &ann.span,
                            "this pattern is not in the regex subset every language supports",
                            "not accepted",
                            Some(format!(
                                "patterns may not use lookaround or backreferences. {}",
                                e.to_string()
                                    .lines()
                                    .last()
                                    .unwrap_or_default()
                                    .trim()
                                    .trim_start_matches("error: ")
                            )),
                        );
                    } else {
                        out.rules.push(Rule::Pattern { regex: re.clone() });
                    }
                }
                ("pattern", _) => bad_args(self, "a regex string: `@pattern(\"^[a-z]+$\")`"),

                ("format", [AnnotationArg::Ident(f)]) => {
                    let found = Format::ALL.iter().copied().find(|x| x.name() == f);
                    match (found, &cls) {
                        (None, _) => self.error(
                            &ann.span,
                            format!("unknown format `{f}`"),
                            "not a format",
                            Some(format!(
                                "formats are: {}",
                                Format::ALL.map(|x| format!("`{}`", x.name())).join(", ")
                            )),
                        ),
                        (Some(_), Some(c)) if *c != Class::Str => self.error(
                            &ann.span,
                            "`@format` applies to strings",
                            format!("this field is {}", shown.clone().unwrap_or_default()),
                            None,
                        ),
                        (Some(format), _) => out.rules.push(Rule::Format { format }),
                    }
                }
                ("format", _) => bad_args(self, "a format name: `@format(email)`"),
                _ => {}
            }
        }

        // min must not exceed max
        let get = |want_min: bool| {
            out.rules.iter().find_map(|r| match r {
                Rule::Min { value } if want_min => value.parse::<f64>().ok(),
                Rule::Max { value } if !want_min => value.parse::<f64>().ok(),
                _ => None,
            })
        };
        if let (Some(lo), Some(hi)) = (get(true), get(false)) {
            if lo > hi {
                if let Some(ann) = anns.iter().find(|a| a.name.name == "max") {
                    self.error(
                        &ann.span,
                        "`@max` is below `@min`",
                        "no value can satisfy both",
                        None,
                    );
                }
            }
        }
        out
    }
}

fn list(items: &[&str]) -> String {
    items
        .iter()
        .map(|i| format!("`@{i}`"))
        .collect::<Vec<_>>()
        .join(", ")
}
