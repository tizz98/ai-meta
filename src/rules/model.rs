//! Data model shared by the config layer and the rule engine (P2).
//!
//! Guards and architecture signals are identified by stable ids. A profile
//! supplies an ordered default set of each; meta.toml can disable ids, override
//! a guard's severity, tune signal thresholds, and append custom grep guards.
//! The engine (P2) maps each id to its implementation; this module is just the
//! vocabulary so config resolution is testable on its own.

use std::fmt;
use std::str::FromStr;

/// A guard verdict (runtime) — also the configurable trip-severity for a guard
/// (only `Warn`/`Fail` are meaningful as configuration; `Pass`/`Skip` are
/// produced at runtime).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Pass,
    Skip,
    Warn,
    Fail,
}

impl Severity {
    pub fn badge(self) -> &'static str {
        match self {
            Severity::Pass => "PASS",
            Severity::Skip => "SKIP",
            Severity::Warn => "WARN",
            Severity::Fail => "FAIL",
        }
    }
}

impl FromStr for Severity {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "pass" => Ok(Severity::Pass),
            "skip" => Ok(Severity::Skip),
            "warn" => Ok(Severity::Warn),
            "fail" => Ok(Severity::Fail),
            other => Err(format!("invalid severity {other:?} (expected pass|skip|warn|fail)")),
        }
    }
}

/// An architecture-signal level. `Ticket` recommends filing a refactor task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalLevel {
    Ok,
    Skip,
    Watch,
    Ticket,
}

impl SignalLevel {
    pub fn badge(self) -> &'static str {
        match self {
            SignalLevel::Ok => "OK",
            SignalLevel::Skip => "SKIP",
            SignalLevel::Watch => "WATCH",
            SignalLevel::Ticket => "TICKET",
        }
    }
}

/// Built-in guard identifiers across all profiles. Implementations live in P2;
/// a guard's applicability is decided by the profile that lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuardId {
    // rust
    NoPanicInLib,
    NoBlockingInAsync,
    NoDbgInLib,
    // typescript
    StrictTsconfig,
    NoDebugger,
    NoTsIgnore,
    NoConsoleLog,
    // python
    NoPrintInLib,
    NoBareExcept,
    // shared
    NoFocusedTests,
    DepsJustified,
}

impl GuardId {
    /// The stable kebab-case name used in meta.toml (`disabled`, severity map).
    pub fn name(self) -> &'static str {
        match self {
            GuardId::NoPanicInLib => "no-panic-in-lib",
            GuardId::NoBlockingInAsync => "no-blocking-in-async",
            GuardId::NoDbgInLib => "no-dbg-in-lib",
            GuardId::StrictTsconfig => "strict-tsconfig",
            GuardId::NoDebugger => "no-debugger",
            GuardId::NoTsIgnore => "no-ts-ignore",
            GuardId::NoConsoleLog => "no-console-log",
            GuardId::NoPrintInLib => "no-print-in-lib",
            GuardId::NoBareExcept => "no-bare-except",
            GuardId::NoFocusedTests => "no-focused-tests",
            GuardId::DepsJustified => "deps-justified",
        }
    }

    pub fn all() -> &'static [GuardId] {
        use GuardId::*;
        &[
            NoPanicInLib,
            NoBlockingInAsync,
            NoDbgInLib,
            StrictTsconfig,
            NoDebugger,
            NoTsIgnore,
            NoConsoleLog,
            NoPrintInLib,
            NoBareExcept,
            NoFocusedTests,
            DepsJustified,
        ]
    }
}

impl FromStr for GuardId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        GuardId::all()
            .iter()
            .copied()
            .find(|g| g.name() == s)
            .ok_or_else(|| format!("unknown guard {s:?}"))
    }
}

impl fmt::Display for GuardId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Built-in architecture-signal identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalId {
    OversizedFiles,
    MassiveModule,
    Fragility,
    UnsafeBlocks,
    DeepNesting,
    CfgForks,
    DebtMarkers,
}

impl SignalId {
    pub fn name(self) -> &'static str {
        match self {
            SignalId::OversizedFiles => "oversized-files",
            SignalId::MassiveModule => "massive-module",
            SignalId::Fragility => "fragility",
            SignalId::UnsafeBlocks => "unsafe-blocks",
            SignalId::DeepNesting => "deep-nesting",
            SignalId::CfgForks => "cfg-forks",
            SignalId::DebtMarkers => "debt-markers",
        }
    }

    pub fn all() -> &'static [SignalId] {
        use SignalId::*;
        &[
            OversizedFiles,
            MassiveModule,
            Fragility,
            UnsafeBlocks,
            DeepNesting,
            CfgForks,
            DebtMarkers,
        ]
    }
}

impl FromStr for SignalId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SignalId::all()
            .iter()
            .copied()
            .find(|g| g.name() == s)
            .ok_or_else(|| format!("unknown signal {s:?}"))
    }
}

impl fmt::Display for SignalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Tunable architecture thresholds (the bash `ARCH_*` knobs). Defaults are
/// profile-supplied; meta.toml's `[rules.signals.thresholds]` overrides any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
    pub file_watch: u32,
    pub file_ticket: u32,
    pub module_watch: u32,
    pub module_ticket: u32,
    pub fragility_watch: u32,
    pub unsafe_watch: u32,
    pub nesting_cols: u32,
    pub nesting_watch: u32,
    pub cfg_fork_watch: u32,
    pub debt_file_watch: u32,
    pub debt_total_ticket: u32,
}

impl Default for Thresholds {
    fn default() -> Self {
        // The realtime-rs (Rust) defaults; profiles may relax some.
        Thresholds {
            file_watch: 400,
            file_ticket: 600,
            module_watch: 300,
            module_ticket: 500,
            fragility_watch: 8,
            unsafe_watch: 3,
            nesting_cols: 24,
            nesting_watch: 18,
            cfg_fork_watch: 6,
            debt_file_watch: 5,
            debt_total_ticket: 25,
        }
    }
}

/// A resolved guard to run: a built-in id with its effective severity, or a
/// custom project-defined grep guard from meta.toml.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedGuard {
    Builtin { id: GuardId, severity: Severity },
    Custom(CustomGuard),
}

/// A project-specific grep guard (the meta.toml escape hatch) — replaces ad-hoc
/// bash guards without forking the binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomGuard {
    pub name: String,
    pub pattern: String,
    pub roots: Vec<String>,
    pub exclude_tests: bool,
    pub severity: Severity,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_ids_roundtrip_through_name() {
        for g in GuardId::all() {
            assert_eq!(GuardId::from_str(g.name()).unwrap(), *g);
        }
    }

    #[test]
    fn signal_ids_roundtrip_through_name() {
        for s in SignalId::all() {
            assert_eq!(SignalId::from_str(s.name()).unwrap(), *s);
        }
    }

    #[test]
    fn severity_parses() {
        assert_eq!(Severity::from_str("FAIL").unwrap(), Severity::Fail);
        assert_eq!(Severity::from_str("warn").unwrap(), Severity::Warn);
        assert!(Severity::from_str("bogus").is_err());
    }

    #[test]
    fn unknown_ids_error() {
        assert!(GuardId::from_str("nope").is_err());
        assert!(SignalId::from_str("nope").is_err());
    }
}
