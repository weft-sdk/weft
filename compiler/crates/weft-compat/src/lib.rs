//! Breaking-change detection: compare two versions of a package's IR.
//!
//! Every change is *safe*, *risky* or *breaking*, judged by who sends and who receives the data (see
//! `direction`). A package's version has to cover its breaking changes: a major bump, or a minor bump while the
//! major version is 0.

mod diff;
mod direction;
mod version;

use std::collections::BTreeSet;

pub use version::{Bump, Version};
use weft_ir::Ir;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Nobody has to change anything.
    Safe,
    /// Compiles and runs, but a receiver may not expect it.
    Risky,
    /// Existing senders or receivers stop working.
    Breaking,
}

impl Severity {
    pub fn word(self) -> &'static str {
        match self {
            Severity::Safe => "safe",
            Severity::Risky => "risky",
            Severity::Breaking => "breaking",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub severity: Severity,
    /// Where: `Order.total`, `route createOrder`, `event order.placed`.
    pub location: String,
    pub message: String,
    /// What to do instead, for breaking changes.
    pub hint: Option<String>,
}

/// What comparing one package found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageReport {
    pub package: String,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub changes: Vec<Change>,
    /// The smallest version bump the changes call for.
    pub required: Bump,
    /// The bump the versions show, when both have a version.
    pub actual: Option<Bump>,
}

impl PackageReport {
    pub fn count(&self, severity: Severity) -> usize {
        self.changes
            .iter()
            .filter(|c| c.severity == severity)
            .count()
    }

    pub fn has_breaking(&self) -> bool {
        self.count(Severity::Breaking) > 0
    }

    /// Whether the version change covers what changed. Only breaking changes need to be covered to pass.
    pub fn breaking_is_covered(&self) -> bool {
        if !self.has_breaking() {
            return true;
        }
        let (Some(old), Some(new)) = (&self.old_version, &self.new_version) else {
            return false;
        };
        match (Version::parse(old), Version::parse(new)) {
            (Some(o), Some(n)) => n.covers_breaking_from(&o),
            _ => false,
        }
    }

    /// Whether the version change covers every change, not only the breaking ones.
    pub fn version_is_enough(&self) -> bool {
        match (self.required, self.actual) {
            (Bump::None, _) => true,
            (Bump::Major, _) => self.breaking_is_covered(),
            (need, Some(have)) => have >= need,
            (_, None) => false,
        }
    }

    /// Whether `weft check` passes for this package.
    pub fn passes(&self) -> bool {
        self.breaking_is_covered()
    }
}

/// Compare every package. Packages are matched by name. A package that disappeared is a breaking change, and a new
/// one is safe.
pub fn compare(old: &[Ir], new: &[Ir]) -> Vec<PackageReport> {
    let names: BTreeSet<&str> = old.iter().chain(new).map(|i| i.package.as_str()).collect();
    names
        .into_iter()
        .map(|name| {
            let o = old.iter().find(|i| i.package == name);
            let n = new.iter().find(|i| i.package == name);
            match (o, n) {
                (Some(o), Some(n)) => compare_package(o, n),
                (Some(o), None) => PackageReport {
                    package: name.to_string(),
                    old_version: o.version.clone(),
                    new_version: None,
                    changes: vec![Change {
                        severity: Severity::Breaking,
                        location: name.to_string(),
                        message: format!("package `{name}` was removed"),
                        hint: Some("other services may import it; keep it, or remove it in the next major version".into()),
                    }],
                    required: Bump::Major,
                    actual: None,
                },
                (None, Some(n)) => PackageReport {
                    package: name.to_string(),
                    old_version: None,
                    new_version: n.version.clone(),
                    changes: vec![Change {
                        severity: Severity::Safe,
                        location: name.to_string(),
                        message: format!("package `{name}` is new"),
                        hint: None,
                    }],
                    required: Bump::None,
                    actual: None,
                },
                (None, None) => unreachable!("the name came from one of the lists"),
            }
        })
        .collect()
}

/// Compare two versions of one package.
pub fn compare_package(old: &Ir, new: &Ir) -> PackageReport {
    let changes = diff::diff(old, new);
    let required = if changes.iter().any(|c| c.severity == Severity::Breaking) {
        Bump::Major
    } else if !changes.is_empty() {
        Bump::Minor
    } else if old.types != new.types
        || old.errors != new.errors
        || old.events != new.events
        || old.routes != new.routes
        || old.rpc_services != new.rpc_services
        || old.graphql != new.graphql
        || old.sockets != new.sockets
    {
        // Something differs that is not a change in meaning: documentation.
        Bump::Patch
    } else {
        Bump::None
    };
    let actual = match (&old.version, &new.version) {
        (Some(o), Some(n)) => match (Version::parse(o), Version::parse(n)) {
            (Some(o), Some(n)) => Some(n.bump_from(&o)),
            _ => None,
        },
        _ => None,
    };
    PackageReport {
        package: new.package.clone(),
        old_version: old.version.clone(),
        new_version: new.version.clone(),
        changes,
        required,
        actual,
    }
}
