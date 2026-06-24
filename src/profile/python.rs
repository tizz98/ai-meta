//! The `python` profile — ruff/pytest/mypy defaults. Detection refines these to
//! the tools actually present in the repo; these are the sensible fallbacks.

use super::{
    default_label_colors, default_statuses, default_types, Commands, CoverageTool, Profile,
    ProfileKind, VersionLocation,
};
use crate::rules::model::{GuardId, Severity, SignalId, Thresholds};

pub fn profile() -> Profile {
    Profile {
        kind: ProfileKind::Python,
        commands: Commands {
            build: None,
            test: Some("pytest".into()),
            fmt: Some("ruff format --check".into()),
            lint: Some("ruff check".into()),
            typecheck: Some("mypy .".into()),
            coverage: Some("pytest --cov".into()),
        },
        coverage_tool: CoverageTool::PytestCov,
        coverage_min: 0,
        coverage_summary: None,
        statuses: default_statuses(),
        types: default_types(),
        guards: vec![
            (GuardId::NoPrintInLib, Severity::Warn),
            (GuardId::NoBareExcept, Severity::Warn),
            (GuardId::NoFocusedTests, Severity::Warn),
            (GuardId::DepsJustified, Severity::Warn),
        ],
        signals: vec![
            SignalId::OversizedFiles,
            SignalId::Fragility,
            SignalId::DeepNesting,
            SignalId::DebtMarkers,
        ],
        thresholds: Thresholds::default(),
        version_location: VersionLocation {
            path: "pyproject.toml".into(),
            anchor: r"^version\s*=".into(),
        },
        source_exts: vec!["py".into()],
        label_colors: default_label_colors(),
    }
}
