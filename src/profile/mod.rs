//! Language **profiles** — the batteries-included defaults baked into the binary
//! so a repo's meta.toml stays tiny. A profile supplies default commands, the
//! coverage tool, default statuses/types, the ordered guard/signal sets, the
//! architecture thresholds, the version-location anchor, and the source-file
//! extensions the rule engine and detector use.

pub mod python;
pub mod rust;
pub mod swift;
pub mod typescript;

use crate::error::{Error, Result};
use crate::rules::model::{GuardId, Severity, SignalId, Thresholds};

/// The supported profiles. `Generic` is the fallback for an unrecognized repo —
/// language-agnostic signals only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    Rust,
    TypeScript,
    Python,
    Swift,
    Generic,
}

impl ProfileKind {
    pub fn name(self) -> &'static str {
        match self {
            ProfileKind::Rust => "rust",
            ProfileKind::TypeScript => "typescript",
            ProfileKind::Python => "python",
            ProfileKind::Swift => "swift",
            ProfileKind::Generic => "generic",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "rust" => Ok(ProfileKind::Rust),
            "typescript" | "ts" | "javascript" | "js" => Ok(ProfileKind::TypeScript),
            "python" | "py" => Ok(ProfileKind::Python),
            "swift" | "swiftpm" => Ok(ProfileKind::Swift),
            "generic" => Ok(ProfileKind::Generic),
            other => Err(Error::UnknownProfile(other.to_string())),
        }
    }
}

/// Which tool produces coverage, selecting the parser the rule engine uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageTool {
    CargoLlvmCov,
    Vitest,
    PytestCov,
    None,
}

/// Default build/test/lint invocations. `None` means the profile has no such
/// step (e.g. Rust has no separate `typecheck`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Commands {
    pub build: Option<String>,
    pub test: Option<String>,
    pub fmt: Option<String>,
    pub lint: Option<String>,
    pub typecheck: Option<String>,
    pub coverage: Option<String>,
}

/// Where the project's version is recorded, for `meta tag`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionLocation {
    pub path: String,
    pub anchor: String,
}

/// A fully-resolved profile: the defaults a repo inherits.
#[derive(Debug, Clone)]
pub struct Profile {
    pub kind: ProfileKind,
    pub commands: Commands,
    pub coverage_tool: CoverageTool,
    pub coverage_min: u32,
    pub coverage_summary: Option<String>,
    pub statuses: Vec<String>,
    pub types: Vec<String>,
    /// Ordered built-in guards with their default trip-severity.
    pub guards: Vec<(GuardId, Severity)>,
    /// Ordered built-in signals.
    pub signals: Vec<SignalId>,
    pub thresholds: Thresholds,
    pub version_location: VersionLocation,
    /// Source-file extensions (no dot) the rule engine scans.
    pub source_exts: Vec<String>,
    /// Default label palette (full label name -> hex color, no `#`).
    pub label_colors: Vec<(String, String)>,
}

impl Profile {
    /// Build the profile for a kind.
    pub fn for_kind(kind: ProfileKind) -> Profile {
        match kind {
            ProfileKind::Rust => rust::profile(),
            ProfileKind::TypeScript => typescript::profile(),
            ProfileKind::Python => python::profile(),
            ProfileKind::Swift => swift::profile(),
            ProfileKind::Generic => generic(),
        }
    }
}

/// Default statuses/types shared by every profile (order matters: index 0 is the
/// default applied to new tasks).
pub fn default_statuses() -> Vec<String> {
    ["todo", "in-progress", "blocked", "done"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

pub fn default_types() -> Vec<String> {
    ["feature", "bug", "chore", "test", "docs"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// The status/type label palette every profile starts from.
pub fn default_label_colors() -> Vec<(String, String)> {
    [
        ("status:todo", "ededed"),
        ("status:in-progress", "0e8a16"),
        ("status:blocked", "b60205"),
        ("status:done", "6f42c1"),
        ("type:feature", "1d76db"),
        ("type:bug", "d73a4a"),
        ("type:chore", "c5def5"),
        ("type:test", "bfd4f2"),
        ("type:docs", "0075ca"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

/// The language-agnostic fallback profile.
fn generic() -> Profile {
    Profile {
        kind: ProfileKind::Generic,
        commands: Commands::default(),
        coverage_tool: CoverageTool::None,
        coverage_min: 0,
        coverage_summary: None,
        statuses: default_statuses(),
        types: default_types(),
        guards: vec![(GuardId::NoFocusedTests, Severity::Warn)],
        signals: vec![
            SignalId::OversizedFiles,
            SignalId::DeepNesting,
            SignalId::DebtMarkers,
        ],
        thresholds: Thresholds::default(),
        version_location: VersionLocation {
            path: "VERSION".to_string(),
            anchor: "^".to_string(),
        },
        source_exts: vec![],
        label_colors: default_label_colors(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases() {
        assert_eq!(ProfileKind::parse("ts").unwrap(), ProfileKind::TypeScript);
        assert_eq!(ProfileKind::parse("PY").unwrap(), ProfileKind::Python);
        assert_eq!(ProfileKind::parse("Rust").unwrap(), ProfileKind::Rust);
        assert_eq!(ProfileKind::parse("Swift").unwrap(), ProfileKind::Swift);
        assert_eq!(ProfileKind::parse("swiftpm").unwrap(), ProfileKind::Swift);
        assert!(ProfileKind::parse("cobol").is_err());
    }

    #[test]
    fn every_kind_builds_a_profile() {
        for k in [
            ProfileKind::Rust,
            ProfileKind::TypeScript,
            ProfileKind::Python,
            ProfileKind::Swift,
            ProfileKind::Generic,
        ] {
            let p = Profile::for_kind(k);
            assert_eq!(p.kind, k);
            assert_eq!(p.statuses[0], "todo");
        }
    }
}
