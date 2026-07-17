//! The `swift` profile — SwiftPM defaults (`swift build`/`swift test`) for a
//! Swift package or macOS/iOS app. Generated CI runs on a macOS runner (Xcode
//! and the Swift toolchain are preinstalled there), so no toolchain-setup step
//! is emitted. Xcode-project repos override the SwiftPM commands in meta.toml.

use super::{
    default_label_colors, default_statuses, default_types, Commands, CoverageTool, Profile,
    ProfileKind, VersionLocation,
};
use crate::rules::model::{GuardId, Severity, SignalId, Thresholds};

pub fn profile() -> Profile {
    Profile {
        kind: ProfileKind::Swift,
        commands: Commands {
            build: Some("swift build".into()),
            test: Some("swift test".into()),
            // `swift format` / SwiftLint aren't guaranteed to be installed, so
            // the profile leaves fmt/lint unset; a repo opts in via meta.toml.
            fmt: None,
            lint: None,
            typecheck: None,
            coverage: Some("swift test --enable-code-coverage".into()),
        },
        // No in-engine coverage parser for Swift yet; the command is still handy
        // for `meta test --coverage`, but coverage isn't gated.
        coverage_tool: CoverageTool::None,
        coverage_min: 0,
        coverage_summary: None,
        statuses: default_statuses(),
        types: default_types(),
        guards: vec![
            (GuardId::NoPrintInLib, Severity::Warn),
            (GuardId::DepsJustified, Severity::Warn),
        ],
        signals: vec![
            SignalId::OversizedFiles,
            SignalId::Fragility,
            SignalId::DeepNesting,
            SignalId::DebtMarkers,
        ],
        thresholds: Thresholds::default(),
        // Swift has no canonical in-repo version file; default to a plain VERSION
        // file (like the generic profile). Projects point `meta tag` elsewhere
        // (e.g. an Xcode MARKETING_VERSION) via `[[version.locations]]`.
        version_location: VersionLocation {
            path: "VERSION".into(),
            anchor: "^".into(),
        },
        source_exts: vec!["swift".into()],
        label_colors: default_label_colors(),
    }
}
