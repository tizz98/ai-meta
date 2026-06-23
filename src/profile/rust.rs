//! The `rust` profile — Cargo workspace defaults, matching realtime-rs's
//! codified `standards.sh`/`guards.sh`/`architecture.sh` (minus the
//! realtime-specific domain-boundary guards, which a repo adds as custom guards).

use super::{
    default_label_colors, default_statuses, default_types, Commands, CoverageTool, Profile,
    ProfileKind, VersionLocation,
};
use crate::rules::model::{GuardId, Severity, SignalId, Thresholds};

pub fn profile() -> Profile {
    Profile {
        kind: ProfileKind::Rust,
        commands: Commands {
            build: Some("cargo build --workspace".into()),
            test: Some("cargo test --workspace".into()),
            fmt: Some("cargo fmt --all --check".into()),
            lint: Some("cargo clippy --workspace --all-targets -- -D warnings".into()),
            typecheck: None,
            coverage: Some("cargo llvm-cov --workspace".into()),
        },
        coverage_tool: CoverageTool::CargoLlvmCov,
        coverage_min: 80,
        coverage_summary: None,
        statuses: default_statuses(),
        types: default_types(),
        guards: vec![
            (GuardId::NoPanicInLib, Severity::Warn),
            (GuardId::NoBlockingInAsync, Severity::Warn),
            (GuardId::NoDbgInLib, Severity::Warn),
            (GuardId::DepsJustified, Severity::Warn),
        ],
        signals: vec![
            SignalId::OversizedFiles,
            SignalId::MassiveModule,
            SignalId::Fragility,
            SignalId::UnsafeBlocks,
            SignalId::DeepNesting,
            SignalId::CfgForks,
            SignalId::DebtMarkers,
        ],
        thresholds: Thresholds::default(),
        version_location: VersionLocation {
            path: "Cargo.toml".into(),
            anchor: r"^version\s*=".into(),
        },
        source_exts: vec!["rs".into()],
        label_colors: default_label_colors(),
    }
}
