//! Loading and validating `.meta/meta.toml`, then resolving it against the
//! baked profile defaults into an [`EffectiveConfig`].

pub mod defaults;
pub mod migrate;
pub mod schema;

pub use defaults::EffectiveConfig;
pub use schema::MetaFile;

use crate::error::{Error, Result};
use crate::profile::{Profile, ProfileKind};
use crate::version::SCHEMA_VERSION;
use std::path::Path;

/// Path to the config file under a repo root.
pub fn config_path(root: &Path) -> std::path::PathBuf {
    root.join(".meta").join("meta.toml")
}

/// Load + validate + resolve the config at `root`.
pub fn load(root: &Path) -> Result<EffectiveConfig> {
    let path = config_path(root);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| Error::Config(format!("cannot read {}: {e}", path.display())))?;
    let cfg = load_from_str(root, &text)?;
    // Cheap per-invocation compatibility nudge (file-based loads only).
    crate::sync::compat::runtime_note(&cfg.framework_version);
    Ok(cfg)
}

/// Resolve config from in-memory TOML text (the testable core of [`load`]).
pub fn load_from_str(root: &Path, text: &str) -> Result<EffectiveConfig> {
    let file = MetaFile::parse(text).map_err(|e| Error::Config(format!("meta.toml: {e}")))?;

    // Fail safe when the repo's config is newer than this binary understands.
    if let Some(v) = file.meta.schema_version {
        if v > SCHEMA_VERSION {
            return Err(Error::SchemaTooNew {
                repo: v,
                binary: SCHEMA_VERSION,
            });
        }
    }

    let kind = match &file.meta.profile {
        Some(name) => ProfileKind::parse(name)?,
        None => ProfileKind::Generic,
    };
    let profile = Profile::for_kind(kind);
    Ok(defaults::merge(root.to_path_buf(), &profile, &file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::CoverageTool;
    use crate::rules::model::{GuardId, ResolvedGuard, Severity, SignalId};
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from("/tmp/x")
    }

    #[test]
    fn minimal_rust_inherits_profile_defaults() {
        let cfg = load_from_str(
            &root(),
            r#"
            [meta]
            profile = "rust"
            [project]
            title = "realtime-rs"
        "#,
        )
        .unwrap();
        assert_eq!(cfg.title, "realtime-rs");
        assert_eq!(cfg.board, "realtime-rs Roadmap");
        assert_eq!(cfg.build.as_deref(), Some("cargo build --workspace"));
        assert_eq!(cfg.coverage_min, 80);
        assert!(matches!(cfg.coverage_tool, CoverageTool::CargoLlvmCov));
        assert_eq!(cfg.version_locations[0].path, "Cargo.toml");
        // 4 default rust guards.
        assert_eq!(cfg.guards.len(), 4);
        assert_eq!(cfg.signals.len(), 7);
    }

    #[test]
    fn typescript_report_only_coverage() {
        let cfg = load_from_str(
            &root(),
            r#"
            [meta]
            profile = "typescript"
            [project]
            title = "ptcg-ai"
        "#,
        )
        .unwrap();
        assert_eq!(cfg.coverage_min, 0);
        assert_eq!(cfg.test.as_deref(), Some("npm test"));
        assert_eq!(cfg.version_locations[0].path, "package.json");
    }

    #[test]
    fn overrides_win_over_profile() {
        let cfg = load_from_str(
            &root(),
            r#"
            [meta]
            profile = "rust"
            [project]
            title = "rt"
            [commands]
            build = "cargo build -p server"
            [coverage]
            min = 90
        "#,
        )
        .unwrap();
        assert_eq!(cfg.build.as_deref(), Some("cargo build -p server"));
        assert_eq!(cfg.coverage_min, 90);
    }

    #[test]
    fn disables_and_reseverities_guards_and_appends_custom() {
        let cfg = load_from_str(
            &root(),
            r#"
            [meta]
            profile = "rust"
            [project]
            title = "rt"
            [rules.guards]
            disabled = ["no-dbg-in-lib"]
            [rules.guards.severity]
            no-panic-in-lib = "fail"
            [[rules.custom_guards]]
            name = "no-rdkafka-in-core"
            pattern = "use rdkafka"
            roots = ["core", "server"]
            severity = "fail"
            message = "Backends only through the trait."
        "#,
        )
        .unwrap();
        // one disabled, custom appended -> 3 builtin + 1 custom
        let builtins: Vec<_> = cfg
            .guards
            .iter()
            .filter_map(|g| match g {
                ResolvedGuard::Builtin { id, severity } => Some((*id, *severity)),
                _ => None,
            })
            .collect();
        assert!(!builtins.iter().any(|(id, _)| *id == GuardId::NoDbgInLib));
        assert!(builtins
            .iter()
            .any(|(id, sev)| *id == GuardId::NoPanicInLib && *sev == Severity::Fail));
        assert!(cfg
            .guards
            .iter()
            .any(|g| matches!(g, ResolvedGuard::Custom(c) if c.name == "no-rdkafka-in-core")));
    }

    #[test]
    fn disables_signal_and_tunes_threshold() {
        let cfg = load_from_str(
            &root(),
            r#"
            [meta]
            profile = "rust"
            [project]
            title = "rt"
            [rules.signals]
            disabled = ["unsafe-blocks"]
            [rules.signals.thresholds]
            file_ticket = 800
        "#,
        )
        .unwrap();
        assert!(!cfg.signals.contains(&SignalId::UnsafeBlocks));
        assert_eq!(cfg.thresholds.file_ticket, 800);
        assert_eq!(cfg.thresholds.file_watch, 400); // untouched default
    }

    #[test]
    fn schema_newer_than_binary_is_rejected() {
        let err = load_from_str(
            &root(),
            r#"
            [meta]
            profile = "rust"
            schema_version = 9999
        "#,
        )
        .unwrap_err();
        assert!(matches!(err, Error::SchemaTooNew { .. }));
    }

    #[test]
    fn unknown_key_is_rejected() {
        let err = load_from_str(
            &root(),
            r#"
            [meta]
            profile = "rust"
            bogus_key = 1
        "#,
        )
        .unwrap_err();
        assert!(matches!(err, Error::Config(_)));
    }
}
