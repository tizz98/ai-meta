//! Framework + config-schema versioning.
//!
//! Two axes, both baked into the binary and recorded in a repo's meta.toml:
//!   - `FRAMEWORK_VERSION` — the ai-meta release (semver, == crate version).
//!   - `SCHEMA_VERSION`    — integer shape of meta.toml; bumped only on a
//!     breaking config change, with a migration in `config::migrate`.

use crate::error::{Error, Result};
use std::cmp::Ordering;
use std::fmt;

/// The ai-meta release this binary is. Mirrored into a repo's `.meta/version`
/// and `meta.framework_version` by `init`/`upgrade`.
pub const FRAMEWORK_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The meta.toml schema version this binary understands.
pub const SCHEMA_VERSION: u32 = 1;

/// A minimal `X.Y.Z` semantic version (no pre-release/build metadata — the
/// framework only ever cuts plain releases).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse `X.Y.Z`, tolerating a leading `v`.
    pub fn parse(s: &str) -> Result<Self> {
        let raw = s.trim().strip_prefix('v').unwrap_or(s.trim());
        let mut it = raw.split('.');
        let mut next = || -> Result<u64> {
            it.next()
                .ok_or_else(|| Error::BadVersion(s.to_string()))?
                .parse::<u64>()
                .map_err(|_| Error::BadVersion(s.to_string()))
        };
        let v = Version::new(next()?, next()?, next()?);
        if it.next().is_some() {
            return Err(Error::BadVersion(s.to_string()));
        }
        Ok(v)
    }

    /// Bump by release level. Returns the next version.
    pub fn bump(self, level: BumpLevel) -> Self {
        match level {
            BumpLevel::Major => Version::new(self.major + 1, 0, 0),
            BumpLevel::Minor => Version::new(self.major, self.minor + 1, 0),
            BumpLevel::Patch => Version::new(self.major, self.minor, self.patch + 1),
        }
    }

    /// This binary's framework version.
    pub fn framework() -> Self {
        // FRAMEWORK_VERSION comes from Cargo and is always valid semver.
        Version::parse(FRAMEWORK_VERSION).unwrap_or(Version::new(0, 0, 0))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

/// Release level for `meta tag` and explicit version bumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpLevel {
    Major,
    Minor,
    Patch,
}

impl BumpLevel {
    /// Parse `major|minor|patch`. Returns `None` for anything else (the caller
    /// then tries to parse it as an explicit `vX.Y.Z`).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "major" => Some(BumpLevel::Major),
            "minor" => Some(BumpLevel::Minor),
            "patch" => Some(BumpLevel::Patch),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_with_and_without_v() {
        assert_eq!(Version::parse("1.2.3").unwrap(), Version::new(1, 2, 3));
        assert_eq!(Version::parse("v0.10.0").unwrap(), Version::new(0, 10, 0));
        assert_eq!(Version::parse("  1.0.0 ").unwrap(), Version::new(1, 0, 0));
    }

    #[test]
    fn rejects_malformed() {
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1.2.3.4").is_err());
        assert!(Version::parse("a.b.c").is_err());
        assert!(Version::parse("1.2.x").is_err());
    }

    #[test]
    fn orders_correctly() {
        assert!(Version::new(1, 0, 0) > Version::new(0, 9, 9));
        assert!(Version::new(1, 2, 0) > Version::new(1, 1, 9));
        assert!(Version::new(1, 1, 2) > Version::new(1, 1, 1));
        assert_eq!(Version::new(1, 1, 1), Version::new(1, 1, 1));
    }

    #[test]
    fn bumps_each_level() {
        let v = Version::new(1, 4, 2);
        assert_eq!(v.bump(BumpLevel::Major), Version::new(2, 0, 0));
        assert_eq!(v.bump(BumpLevel::Minor), Version::new(1, 5, 0));
        assert_eq!(v.bump(BumpLevel::Patch), Version::new(1, 4, 3));
    }
}
