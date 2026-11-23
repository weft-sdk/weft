//! Semantic versions, as far as `weft check` needs them.

/// How far a version moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bump {
    None,
    Patch,
    Minor,
    Major,
}

impl Bump {
    /// A name for machines: `none`, `patch`, `minor` or `major`.
    pub fn id(self) -> &'static str {
        match self {
            Bump::None => "none",
            Bump::Patch => "patch",
            Bump::Minor => "minor",
            Bump::Major => "major",
        }
    }

    /// A phrase for people: `a major`, as in "needs a major version bump".
    pub fn word(self) -> &'static str {
        match self {
            Bump::None => "no",
            Bump::Patch => "a patch",
            Bump::Minor => "a minor",
            Bump::Major => "a major",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    /// `MAJOR.MINOR.PATCH`, ignoring a pre-release or build suffix.
    pub fn parse(text: &str) -> Option<Version> {
        let core = text.split(['-', '+']).next()?;
        let mut parts = core.split('.');
        let v = Version {
            major: parts.next()?.parse().ok()?,
            minor: parts.next()?.parse().ok()?,
            patch: parts.next()?.parse().ok()?,
        };
        parts.next().is_none().then_some(v)
    }

    /// How far this version is above `old`. A version that went down or stayed put is no bump.
    pub fn bump_from(&self, old: &Version) -> Bump {
        if self <= old {
            Bump::None
        } else if self.major > old.major {
            Bump::Major
        } else if self.minor > old.minor {
            Bump::Minor
        } else {
            Bump::Patch
        }
    }

    /// Whether going from `old` to this version allows breaking changes: a major bump, or a minor bump while the
    /// major version is 0 (semver: anything may change before 1.0).
    pub fn covers_breaking_from(&self, old: &Version) -> bool {
        match self.bump_from(old) {
            Bump::Major => true,
            Bump::Minor => old.major == 0 && self.major == 0,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_compares() {
        let v = |s| Version::parse(s).unwrap();
        assert_eq!(
            v("1.2.3-beta+5"),
            Version {
                major: 1,
                minor: 2,
                patch: 3
            }
        );
        assert!(
            Version::parse("1.2").is_none()
                && Version::parse("1.2.3.4").is_none()
                && Version::parse("a.b.c").is_none()
        );
        assert_eq!(v("2.0.0").bump_from(&v("1.9.9")), Bump::Major);
        assert_eq!(v("1.3.0").bump_from(&v("1.2.9")), Bump::Minor);
        assert_eq!(v("1.2.4").bump_from(&v("1.2.3")), Bump::Patch);
        assert_eq!(v("1.2.3").bump_from(&v("1.2.3")), Bump::None);
        assert_eq!(v("1.0.0").bump_from(&v("2.0.0")), Bump::None);
    }

    #[test]
    fn breaking_needs_a_major_bump_but_zero_x_may_use_minor() {
        let v = |s| Version::parse(s).unwrap();
        assert!(v("2.0.0").covers_breaking_from(&v("1.5.0")));
        assert!(!v("1.6.0").covers_breaking_from(&v("1.5.0")));
        assert!(!v("1.5.1").covers_breaking_from(&v("1.5.0")));
        assert!(v("0.2.0").covers_breaking_from(&v("0.1.0")));
        assert!(!v("0.1.1").covers_breaking_from(&v("0.1.0")));
        assert!(v("1.0.0").covers_breaking_from(&v("0.9.0")));
    }
}
