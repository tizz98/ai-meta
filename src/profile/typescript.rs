//! The `typescript` profile — npm/tsc/vitest defaults, matching ptcg-ai's
//! codified config (coverage reported but not gated by default).

use super::{
    default_label_colors, default_statuses, default_types, Commands, CoverageTool, Profile,
    ProfileKind, VersionLocation,
};
use crate::rules::model::{GuardId, Severity, SignalId, Thresholds};

pub fn profile() -> Profile {
    Profile {
        kind: ProfileKind::TypeScript,
        commands: Commands {
            build: Some("npm run build".into()),
            test: Some("npm test".into()),
            fmt: None,
            lint: Some("npm run lint".into()),
            typecheck: Some("npm run typecheck".into()),
            coverage: Some("npm run test:coverage".into()),
        },
        coverage_tool: CoverageTool::Vitest,
        coverage_min: 0,
        coverage_summary: Some("coverage/coverage-summary.json".into()),
        statuses: default_statuses(),
        types: default_types(),
        guards: vec![
            (GuardId::StrictTsconfig, Severity::Fail),
            (GuardId::NoDebugger, Severity::Fail),
            (GuardId::NoFocusedTests, Severity::Fail),
            (GuardId::NoTsIgnore, Severity::Warn),
            (GuardId::NoConsoleLog, Severity::Warn),
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
            path: "package.json".into(),
            anchor: r#""version"\s*:"#.into(),
        },
        source_exts: vec!["ts".into(), "tsx".into()],
        label_colors: default_label_colors(),
    }
}
