//! The rule engine: the data model, the source scanner, the test-exclusion
//! balancer, the guard/signal implementations, and the runners that drive them
//! from a resolved [`EffectiveConfig`].

pub mod coverage;
pub mod grep;
pub mod guards;
pub mod model;
pub mod nontest;
pub mod signals;

use crate::config::EffectiveConfig;
use guards::{GuardCtx, GuardResult};
use model::{ResolvedGuard, Severity, SignalLevel};
use signals::{Finding, SignalCtx, SignalResult, TicketDraft};

/// One guard's resolved outcome.
#[derive(Debug, Clone)]
pub struct CheckEntry {
    pub name: String,
    pub verdict: Severity,
    pub summary: String,
    pub hits: Vec<String>,
}

/// The aggregate `meta check` result.
#[derive(Debug, Clone, Default)]
pub struct CheckReport {
    pub entries: Vec<CheckEntry>,
    pub pass: usize,
    pub skip: usize,
    pub warn: usize,
    pub fail: usize,
}

impl CheckReport {
    /// Exit code: 1 if any FAIL, or (under strict) any WARN; else 0.
    pub fn exit_code(&self, strict: bool) -> i32 {
        if self.fail > 0 || (strict && self.warn > 0) {
            1
        } else {
            0
        }
    }
}

/// Run all configured guards against the project.
pub fn run_checks(cfg: &EffectiveConfig) -> CheckReport {
    let ctx = GuardCtx {
        root: &cfg.root,
        profile: cfg.profile_kind,
        source_exts: &cfg.source_exts,
        deps_allowlist: &cfg.deps_allowlist,
        deps_doc: cfg.deps_doc.as_deref(),
    };
    let mut report = CheckReport::default();
    for g in &cfg.guards {
        let (name, trip_severity, result) = match g {
            ResolvedGuard::Builtin { id, severity } => {
                (id.name().to_string(), *severity, guards::run(*id, &ctx))
            }
            ResolvedGuard::Custom(c) => (c.name.clone(), c.severity, guards::run_custom(c, &ctx)),
        };
        let entry = match result {
            GuardResult::Skip(summary) => {
                report.skip += 1;
                CheckEntry {
                    name,
                    verdict: Severity::Skip,
                    summary,
                    hits: vec![],
                }
            }
            GuardResult::Pass(summary) => {
                report.pass += 1;
                CheckEntry {
                    name,
                    verdict: Severity::Pass,
                    summary,
                    hits: vec![],
                }
            }
            GuardResult::Trip { summary, hits } => {
                let verdict = match trip_severity {
                    Severity::Fail => {
                        report.fail += 1;
                        Severity::Fail
                    }
                    _ => {
                        report.warn += 1;
                        Severity::Warn
                    }
                };
                CheckEntry {
                    name,
                    verdict,
                    summary,
                    hits,
                }
            }
        };
        report.entries.push(entry);
    }
    report
}

/// One signal's resolved outcome.
#[derive(Debug, Clone)]
pub struct ArchSignalReport {
    pub name: String,
    pub skipped: Option<String>,
    pub clean: Option<String>,
    pub findings: Vec<Finding>,
}

/// The aggregate `meta arch` result.
#[derive(Debug, Clone, Default)]
pub struct ArchReport {
    pub signals: Vec<ArchSignalReport>,
    pub clean: usize,
    pub skip: usize,
    pub watch: usize,
    pub ticket: usize,
    pub tickets: Vec<TicketDraft>,
}

impl ArchReport {
    /// Advisory exit: 2 if any ticket candidates (non-blocking signal), 1 under
    /// strict, else 0.
    pub fn exit_code(&self, strict: bool) -> i32 {
        if self.ticket > 0 {
            if strict {
                1
            } else {
                2
            }
        } else {
            0
        }
    }
}

/// Run all configured architecture signals against the project.
pub fn run_arch(cfg: &EffectiveConfig) -> ArchReport {
    let ctx = SignalCtx {
        root: &cfg.root,
        profile: cfg.profile_kind,
        source_exts: &cfg.source_exts,
        thresholds: cfg.thresholds,
    };
    let mut report = ArchReport::default();
    for id in &cfg.signals {
        let sr = signals::run(*id, &ctx);
        let entry = match sr {
            SignalResult::Skip(reason) => {
                report.skip += 1;
                ArchSignalReport {
                    name: id.name().to_string(),
                    skipped: Some(reason),
                    clean: None,
                    findings: vec![],
                }
            }
            SignalResult::Findings { findings, clean } => {
                if findings.is_empty() {
                    report.clean += 1;
                    ArchSignalReport {
                        name: id.name().to_string(),
                        skipped: None,
                        clean: Some(clean),
                        findings: vec![],
                    }
                } else {
                    for f in &findings {
                        match f.level {
                            SignalLevel::Watch => report.watch += 1,
                            SignalLevel::Ticket => {
                                report.ticket += 1;
                                if let Some(t) = &f.ticket {
                                    report.tickets.push(t.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                    ArchSignalReport {
                        name: id.name().to_string(),
                        skipped: None,
                        clean: None,
                        findings,
                    }
                }
            }
        };
        report.signals.push(entry);
    }
    report
}
