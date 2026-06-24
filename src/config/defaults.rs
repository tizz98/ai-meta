//! Merge a parsed [`MetaFile`] over a [`Profile`]'s baked defaults into an
//! [`EffectiveConfig`] — the concrete, Option-free view every command consumes.
//! Precedence: meta.toml value > profile default. Lists like guards/signals are
//! the profile's set minus `disabled`, with severity/threshold overrides and any
//! custom guards appended.

use super::schema::{CodegenEntry, ExtraGate, ExtraStep, MetaFile};
use crate::output;
use crate::profile::{CoverageTool, Profile, ProfileKind, VersionLocation};
use crate::rules::model::{CustomGuard, GuardId, ResolvedGuard, Severity, SignalId, Thresholds};
use std::path::PathBuf;
use std::str::FromStr;

/// The fully-resolved configuration for a project.
#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    pub root: PathBuf,
    pub schema_version: u32,
    pub framework_version: String,
    pub profile_kind: ProfileKind,

    pub title: String,
    pub board: String,
    pub description: Option<String>,

    pub domains: Vec<String>,
    pub statuses: Vec<String>,
    pub types: Vec<String>,

    pub build: Option<String>,
    pub test: Option<String>,
    pub fmt: Option<String>,
    pub lint: Option<String>,
    pub typecheck: Option<String>,
    pub coverage: Option<String>,

    pub coverage_tool: CoverageTool,
    pub coverage_min: u32,
    pub coverage_summary: Option<String>,

    pub codegen: Vec<CodegenEntry>,
    pub ci_extra_gates: Vec<ExtraGate>,
    pub ci_extra_steps: Vec<ExtraStep>,

    pub milestones: Vec<(String, String)>,
    pub label_colors: Vec<(String, String)>,
    pub label_descriptions: Vec<(String, String)>,

    pub deps_allowlist: Vec<String>,
    pub deps_doc: Option<String>,

    pub version_locations: Vec<VersionLocation>,
    pub default_bump: String,
    pub tag_prefix: String,
    pub require_branch: String,

    pub guards: Vec<ResolvedGuard>,
    pub signals: Vec<SignalId>,
    pub thresholds: Thresholds,
    pub source_exts: Vec<String>,

    pub sync_ignore: Vec<String>,
}

/// Build the effective config for `root` from a profile + parsed file.
pub fn merge(root: PathBuf, profile: &Profile, file: &MetaFile) -> EffectiveConfig {
    let title = file
        .project
        .title
        .clone()
        .unwrap_or_else(|| dir_name(&root));
    let board = file
        .github
        .board
        .clone()
        .unwrap_or_else(|| format!("{title} Roadmap"));

    EffectiveConfig {
        schema_version: file
            .meta
            .schema_version
            .unwrap_or(crate::version::SCHEMA_VERSION),
        framework_version: file
            .meta
            .framework_version
            .clone()
            .unwrap_or_else(|| crate::version::FRAMEWORK_VERSION.to_string()),
        profile_kind: profile.kind,

        title,
        board,
        description: file.project.description.clone(),

        domains: file.github.domains.clone(),
        statuses: non_empty(&file.github.statuses, &profile.statuses),
        types: non_empty(&file.github.types, &profile.types),

        build: file
            .commands
            .build
            .clone()
            .or_else(|| profile.commands.build.clone()),
        test: file
            .commands
            .test
            .clone()
            .or_else(|| profile.commands.test.clone()),
        fmt: file
            .commands
            .fmt
            .clone()
            .or_else(|| profile.commands.fmt.clone()),
        lint: file
            .commands
            .lint
            .clone()
            .or_else(|| profile.commands.lint.clone()),
        typecheck: file
            .commands
            .typecheck
            .clone()
            .or_else(|| profile.commands.typecheck.clone()),
        coverage: file
            .commands
            .coverage
            .clone()
            .or_else(|| profile.commands.coverage.clone()),

        coverage_tool: profile.coverage_tool,
        coverage_min: file.coverage.min.unwrap_or(profile.coverage_min),
        coverage_summary: file
            .coverage
            .summary_path
            .clone()
            .or_else(|| profile.coverage_summary.clone()),

        codegen: file.codegen.clone(),
        ci_extra_gates: file.ci.extra_gates.clone(),
        ci_extra_steps: file.ci.extra_steps.clone(),

        milestones: file
            .milestones
            .iter()
            .map(|m| (m.title.clone(), m.description.clone()))
            .collect(),
        label_colors: merge_label_colors(profile, file),
        label_descriptions: file
            .labels
            .descriptions
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),

        deps_allowlist: file.deps.allowlist.clone(),
        deps_doc: file.deps.doc.clone(),

        version_locations: resolve_version_locations(profile, file),
        default_bump: file
            .version
            .default_bump
            .clone()
            .unwrap_or_else(|| "minor".into()),
        tag_prefix: file
            .version
            .tag_prefix
            .clone()
            .unwrap_or_else(|| "auto".into()),
        require_branch: file
            .version
            .require_branch
            .clone()
            .unwrap_or_else(|| "main".into()),

        guards: resolve_guards(profile, file),
        signals: resolve_signals(profile, file),
        thresholds: resolve_thresholds(profile, file),
        source_exts: profile.source_exts.clone(),

        sync_ignore: file.sync.ignore.clone(),
        root,
    }
}

fn dir_name(root: &std::path::Path) -> String {
    root.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string()
}

fn non_empty(file_list: &[String], default: &[String]) -> Vec<String> {
    if file_list.is_empty() {
        default.to_vec()
    } else {
        file_list.to_vec()
    }
}

fn merge_label_colors(profile: &Profile, file: &MetaFile) -> Vec<(String, String)> {
    let mut out = profile.label_colors.clone();
    for (k, v) in &file.labels.colors {
        if let Some(slot) = out.iter_mut().find(|(name, _)| name == k) {
            slot.1 = v.clone();
        } else {
            out.push((k.clone(), v.clone()));
        }
    }
    out
}

fn resolve_version_locations(profile: &Profile, file: &MetaFile) -> Vec<VersionLocation> {
    if file.version.locations.is_empty() {
        vec![profile.version_location.clone()]
    } else {
        file.version
            .locations
            .iter()
            .map(|l| VersionLocation {
                path: l.path.clone(),
                anchor: l.anchor.clone(),
            })
            .collect()
    }
}

fn resolve_guards(profile: &Profile, file: &MetaFile) -> Vec<ResolvedGuard> {
    let disabled: Vec<GuardId> = file
        .rules
        .guards
        .disabled
        .iter()
        .filter_map(|s| parse_or_warn(GuardId::from_str(s)))
        .collect();

    let mut out: Vec<ResolvedGuard> = profile
        .guards
        .iter()
        .filter(|(id, _)| !disabled.contains(id))
        .map(|(id, default_sev)| {
            let severity = file
                .rules
                .guards
                .severity
                .get(id.name())
                .and_then(|s| parse_or_warn(Severity::from_str(s)))
                .unwrap_or(*default_sev);
            ResolvedGuard::Builtin { id: *id, severity }
        })
        .collect();

    for c in &file.rules.custom_guards {
        let severity = parse_or_warn(Severity::from_str(&c.severity)).unwrap_or(Severity::Warn);
        out.push(ResolvedGuard::Custom(CustomGuard {
            name: c.name.clone(),
            pattern: c.pattern.clone(),
            roots: c.roots.clone(),
            exclude_tests: c.exclude_tests,
            severity,
            message: c.message.clone(),
        }));
    }
    out
}

fn resolve_signals(profile: &Profile, file: &MetaFile) -> Vec<SignalId> {
    let disabled: Vec<SignalId> = file
        .rules
        .signals
        .disabled
        .iter()
        .filter_map(|s| parse_or_warn(SignalId::from_str(s)))
        .collect();
    profile
        .signals
        .iter()
        .copied()
        .filter(|id| !disabled.contains(id))
        .collect()
}

fn resolve_thresholds(profile: &Profile, file: &MetaFile) -> Thresholds {
    let mut t = profile.thresholds;
    let o = &file.rules.signals.thresholds;
    if let Some(v) = o.file_watch {
        t.file_watch = v;
    }
    if let Some(v) = o.file_ticket {
        t.file_ticket = v;
    }
    if let Some(v) = o.module_watch {
        t.module_watch = v;
    }
    if let Some(v) = o.module_ticket {
        t.module_ticket = v;
    }
    if let Some(v) = o.fragility_watch {
        t.fragility_watch = v;
    }
    if let Some(v) = o.unsafe_watch {
        t.unsafe_watch = v;
    }
    if let Some(v) = o.nesting_cols {
        t.nesting_cols = v;
    }
    if let Some(v) = o.nesting_watch {
        t.nesting_watch = v;
    }
    if let Some(v) = o.cfg_fork_watch {
        t.cfg_fork_watch = v;
    }
    if let Some(v) = o.debt_file_watch {
        t.debt_file_watch = v;
    }
    if let Some(v) = o.debt_total_ticket {
        t.debt_total_ticket = v;
    }
    t
}

/// Parse a config-supplied id/severity, warning (and dropping) on a typo rather
/// than failing the whole run — a misspelled rule name shouldn't brick `check`.
fn parse_or_warn<T, E: std::fmt::Display>(r: Result<T, E>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            output::warn(format!("config: {e} (ignored)"));
            None
        }
    }
}
