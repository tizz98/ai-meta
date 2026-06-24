//! The raw, deserialized `.meta/meta.toml`. Every field is optional so an
//! omitted value inherits the profile default (resolved in [`super::defaults`]).
//! This module is just the on-disk shape; it carries no defaults of its own
//! beyond serde's empties.

use serde::Deserialize;
use std::collections::BTreeMap;

/// The full meta.toml document.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetaFile {
    #[serde(default)]
    pub meta: MetaTable,
    #[serde(default)]
    pub project: ProjectTable,
    #[serde(default)]
    pub github: GithubTable,
    #[serde(default)]
    pub commands: CommandsTable,
    #[serde(default)]
    pub coverage: CoverageTable,
    #[serde(default)]
    pub ci: CiTable,
    #[serde(default)]
    pub codegen: Vec<CodegenEntry>,
    #[serde(default)]
    pub milestones: Vec<MilestoneEntry>,
    #[serde(default)]
    pub labels: LabelsTable,
    #[serde(default)]
    pub deps: DepsTable,
    #[serde(default)]
    pub version: VersionTable,
    #[serde(default)]
    pub rules: RulesTable,
    #[serde(default)]
    pub sync: SyncTable,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetaTable {
    pub schema_version: Option<u32>,
    pub framework_version: Option<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectTable {
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GithubTable {
    pub board: Option<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub statuses: Vec<String>,
    #[serde(default)]
    pub types: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandsTable {
    pub build: Option<String>,
    pub test: Option<String>,
    pub fmt: Option<String>,
    pub lint: Option<String>,
    pub typecheck: Option<String>,
    pub coverage: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageTable {
    pub min: Option<u32>,
    pub tool: Option<String>,
    pub summary_path: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiTable {
    #[serde(default)]
    pub extra_gates: Vec<ExtraGate>,
    #[serde(default)]
    pub extra_steps: Vec<ExtraStep>,
}

/// An extra hard/soft CI gate beyond the profile baseline (e.g. SDK builds).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraGate {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub when_dir: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub hard: bool,
}

/// An extra step injected into a generated workflow (e.g. start redis).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtraStep {
    pub workflow: String,
    pub name: String,
    pub run: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodegenEntry {
    pub name: String,
    pub command: String,
    pub trigger: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MilestoneEntry {
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelsTable {
    #[serde(default)]
    pub colors: BTreeMap<String, String>,
    #[serde(default)]
    pub descriptions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepsTable {
    #[serde(default)]
    pub allowlist: Vec<String>,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionTable {
    #[serde(default)]
    pub locations: Vec<VersionLocationEntry>,
    pub default_bump: Option<String>,
    pub tag_prefix: Option<String>,
    pub require_branch: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionLocationEntry {
    pub path: String,
    pub anchor: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesTable {
    #[serde(default)]
    pub guards: GuardsTable,
    #[serde(default)]
    pub signals: SignalsTable,
    #[serde(default)]
    pub custom_guards: Vec<CustomGuardEntry>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardsTable {
    #[serde(default)]
    pub disabled: Vec<String>,
    #[serde(default)]
    pub severity: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalsTable {
    #[serde(default)]
    pub disabled: Vec<String>,
    #[serde(default)]
    pub thresholds: ThresholdsTable,
}

/// All optional — each present field overrides the profile threshold.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdsTable {
    pub file_watch: Option<u32>,
    pub file_ticket: Option<u32>,
    pub module_watch: Option<u32>,
    pub module_ticket: Option<u32>,
    pub fragility_watch: Option<u32>,
    pub unsafe_watch: Option<u32>,
    pub nesting_cols: Option<u32>,
    pub nesting_watch: Option<u32>,
    pub cfg_fork_watch: Option<u32>,
    pub debt_file_watch: Option<u32>,
    pub debt_total_ticket: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomGuardEntry {
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default)]
    pub exclude_tests: bool,
    #[serde(default = "default_custom_severity")]
    pub severity: String,
    #[serde(default)]
    pub message: String,
}

fn default_custom_severity() -> String {
    "warn".to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncTable {
    #[serde(default)]
    pub ignore: Vec<String>,
}

impl MetaFile {
    /// Parse from TOML text.
    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}
