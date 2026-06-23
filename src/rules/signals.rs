//! Built-in architecture-signal implementations — the advisory "tech-debt"
//! heuristics behind `meta arch`. A signal inspects product (non-test) sources
//! and returns OK / WATCH / TICKET findings, with a ready-to-file ticket draft
//! at the ticket threshold. Ports `architecture.sh`.

use super::grep::{self};
use super::model::{SignalId, SignalLevel, Thresholds};
use crate::profile::ProfileKind;
use regex::Regex;
use std::path::Path;

/// One finding within a signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub level: SignalLevel,
    pub summary: String,
    pub detail: String,
    pub ticket: Option<TicketDraft>,
}

/// A drafted refactor ticket (`meta arch` prints a ready-to-run `meta task new`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketDraft {
    pub title: String,
    pub domain: String,
    pub body: String,
}

/// The outcome of running one signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalResult {
    Skip(String),
    /// Findings (possibly empty → clean, using `clean` as the OK summary).
    Findings { findings: Vec<Finding>, clean: String },
}

pub struct SignalCtx<'a> {
    pub root: &'a Path,
    pub profile: ProfileKind,
    pub source_exts: &'a [String],
    pub thresholds: Thresholds,
}

impl SignalCtx<'_> {
    /// Product (non-test) source files.
    fn product_files(&self) -> Vec<std::path::PathBuf> {
        grep::collect_files(self.root, self.source_exts, &[])
            .into_iter()
            .filter(|p| !grep::is_test_path(&grep::rel_display(self.root, p), self.source_exts))
            .collect()
    }
}

/// Run a built-in signal.
pub fn run(id: SignalId, ctx: &SignalCtx) -> SignalResult {
    let files = ctx.product_files();
    if files.is_empty() {
        return SignalResult::Skip("no product sources yet".into());
    }
    let t = ctx.thresholds;
    match id {
        SignalId::OversizedFiles => oversized(ctx, &files, t.file_watch, t.file_ticket),
        SignalId::MassiveModule => massive_module(ctx, &files, t.module_watch, t.module_ticket),
        SignalId::Fragility => fragility(ctx, &files, t.fragility_watch),
        SignalId::UnsafeBlocks => per_file_count(
            ctx,
            &files,
            r"\bunsafe\b",
            t.unsafe_watch,
            "unsafe cluster",
            "unsafe usages",
            "Review for soundness; document each with a // SAFETY: rationale and minimize the unsafe surface.",
            "no concentrated unsafe usage",
        ),
        SignalId::DeepNesting => deep_nesting(ctx, &files, t.nesting_cols, t.nesting_watch),
        SignalId::CfgForks => per_file_count(
            ctx,
            &files,
            r"#\[cfg\(",
            t.cfg_fork_watch,
            "cfg-fork hotspot",
            "#[cfg(...)] blocks",
            "Consider a trait/abstraction boundary instead of scattered conditional compilation.",
            "no #[cfg(...)] hotspots",
        ),
        SignalId::DebtMarkers => debt_markers(ctx, &files, t.debt_file_watch, t.debt_total_ticket),
    }
}

fn oversized(ctx: &SignalCtx, files: &[std::path::PathBuf], watch: u32, ticket: u32) -> SignalResult {
    let mut findings = Vec::new();
    for f in files {
        let rel = grep::rel_display(ctx.root, f);
        let lines = line_count(f);
        if lines >= ticket {
            findings.push(Finding {
                level: SignalLevel::Ticket,
                summary: format!("oversized file: {rel} ({lines} lines ≥ {ticket})"),
                detail: format!("Split responsibilities into cohesive units; target < ~{watch} lines each."),
                ticket: Some(TicketDraft {
                    title: format!("Refactor {} ({lines} lines) into focused modules", base(&rel)),
                    domain: domain_of(&rel),
                    body: format!("Split responsibilities into cohesive modules; target < ~{watch} lines each."),
                }),
            });
        } else if lines >= watch {
            findings.push(Finding {
                level: SignalLevel::Watch,
                summary: format!("large file: {rel} ({lines} lines ≥ {watch})"),
                detail: format!("Approaching the {ticket}-line refactor threshold — watch for further growth."),
                ticket: None,
            });
        }
    }
    SignalResult::Findings {
        findings,
        clean: format!("all product files under {watch} lines"),
    }
}

fn massive_module(
    ctx: &SignalCtx,
    files: &[std::path::PathBuf],
    watch: u32,
    ticket: u32,
) -> SignalResult {
    let mut findings = Vec::new();
    for f in files {
        let rel = grep::rel_display(ctx.root, f);
        let b = base(&rel);
        if b != "mod.rs" && b != "lib.rs" {
            continue;
        }
        let lines = line_count(f);
        if lines >= ticket {
            findings.push(Finding {
                level: SignalLevel::Ticket,
                summary: format!("monolithic module: {rel} ({lines} lines ≥ {ticket})"),
                detail: "Break into cohesive submodules; keep mod.rs/lib.rs a thin re-export surface.".into(),
                ticket: Some(TicketDraft {
                    title: format!("Split {rel} into submodules"),
                    domain: domain_of(&rel),
                    body: "Break the module into cohesive submodules; keep mod.rs/lib.rs a thin re-export surface.".into(),
                }),
            });
        } else if lines >= watch {
            findings.push(Finding {
                level: SignalLevel::Watch,
                summary: format!("large module: {rel} ({lines} lines ≥ {watch})"),
                detail: "Consider extracting submodules before it grows further.".into(),
                ticket: None,
            });
        }
    }
    SignalResult::Findings {
        findings,
        clean: "no oversized modules (mod.rs/lib.rs)".into(),
    }
}

fn fragility(ctx: &SignalCtx, files: &[std::path::PathBuf], watch: u32) -> SignalResult {
    let pattern = match ctx.profile {
        ProfileKind::Rust => r"\.unwrap\(\)|\.expect\(|panic!\(|unreachable!\(",
        ProfileKind::TypeScript => r"\bas any\b|as unknown as|@ts-ignore",
        ProfileKind::Python => r"except\s*:|# type: ignore",
        ProfileKind::Generic => r"$^",
    };
    per_file_count(
        ctx,
        files,
        pattern,
        watch,
        "fragile code",
        "fragile constructs",
        "Prefer typed errors / narrowed types / graceful degradation over force-ops.",
        "no concentrated fragile constructs",
    )
}

fn deep_nesting(ctx: &SignalCtx, files: &[std::path::PathBuf], cols: u32, watch: u32) -> SignalResult {
    let re = compile(&format!(r"^ {{{cols},}}[^ ]"));
    let mut findings = Vec::new();
    for f in files {
        let rel = grep::rel_display(ctx.root, f);
        let n = grep::count_matches(&grep::read(f), &re) as u32;
        if n >= watch {
            findings.push(Finding {
                level: SignalLevel::Watch,
                summary: format!("deep nesting: {rel} ({n} lines indented ≥ {cols} cols)"),
                detail: "Flatten with early-returns / the ? operator / guard clauses; extract helpers.".into(),
                ticket: None,
            });
        }
    }
    SignalResult::Findings {
        findings,
        clean: "no deep-nesting hotspots".into(),
    }
}

fn debt_markers(
    ctx: &SignalCtx,
    files: &[std::path::PathBuf],
    file_watch: u32,
    total_ticket: u32,
) -> SignalResult {
    let re = compile(r"(//|/\*|#).*(TODO|FIXME|HACK|XXX)");
    let mut findings = Vec::new();
    let mut total = 0u32;
    for f in files {
        let rel = grep::rel_display(ctx.root, f);
        let n = grep::count_matches(&grep::read(f), &re) as u32;
        total += n;
        if n >= file_watch {
            findings.push(Finding {
                level: SignalLevel::Watch,
                summary: format!("debt cluster: {rel} ({n} TODO/FIXME/HACK markers)"),
                detail: "Promote real work to tracked tasks; close stale markers.".into(),
                ticket: None,
            });
        }
    }
    if total >= total_ticket {
        findings.push(Finding {
            level: SignalLevel::Ticket,
            summary: format!("high debt-marker load: {total} markers across product code (≥ {total_ticket})"),
            detail: "Triage and burn down TODO/FIXME/HACK debt markers.".into(),
            ticket: Some(TicketDraft {
                title: "Triage and burn down TODO/FIXME/HACK debt markers".into(),
                domain: "core".into(),
                body: "Group related markers, close stale ones, promote real work to tracked tasks.".into(),
            }),
        });
    }
    SignalResult::Findings {
        findings,
        clean: format!("debt markers within limits ({total} total)"),
    }
}

#[allow(clippy::too_many_arguments)]
fn per_file_count(
    ctx: &SignalCtx,
    files: &[std::path::PathBuf],
    pattern: &str,
    watch: u32,
    label: &str,
    noun: &str,
    detail: &str,
    clean: &str,
) -> SignalResult {
    let re = compile(pattern);
    let mut findings = Vec::new();
    for f in files {
        let rel = grep::rel_display(ctx.root, f);
        let n = grep::count_matches(&grep::read(f), &re) as u32;
        if n >= watch {
            findings.push(Finding {
                level: SignalLevel::Watch,
                summary: format!("{label}: {rel} ({n} {noun})"),
                detail: detail.to_string(),
                ticket: None,
            });
        }
    }
    SignalResult::Findings {
        findings,
        clean: clean.to_string(),
    }
}

// --- helpers -----------------------------------------------------------------

fn line_count(path: &Path) -> u32 {
    grep::read(path).lines().count() as u32
}

fn base(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).to_string()
}

/// Best-guess `domain:` value for a drafted ticket — the top-level path segment,
/// matching the bash `_arch_domain` default of `core`.
fn domain_of(rel: &str) -> String {
    let first = rel.split('/').next().unwrap_or("");
    match first {
        "server" => "gateway".to_string(),
        "" | "src" => "core".to_string(),
        other => other.to_string(),
    }
}

fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("built-in signal pattern is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn ctx<'a>(root: &'a Path, exts: &'a [String], profile: ProfileKind) -> SignalCtx<'a> {
        SignalCtx {
            root,
            profile,
            source_exts: exts,
            thresholds: Thresholds::default(),
        }
    }

    #[test]
    fn skips_when_empty() {
        let tmp = tempdir().unwrap();
        let exts = vec!["rs".to_string()];
        assert!(matches!(
            run(SignalId::OversizedFiles, &ctx(tmp.path(), &exts, ProfileKind::Rust)),
            SignalResult::Skip(_)
        ));
    }

    #[test]
    fn oversized_tickets_and_clean() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("core")).unwrap();
        let big = "x\n".repeat(650);
        fs::write(tmp.path().join("core/big.rs"), big).unwrap();
        fs::write(tmp.path().join("core/small.rs"), "fn a(){}\n").unwrap();
        let exts = vec!["rs".to_string()];
        match run(SignalId::OversizedFiles, &ctx(tmp.path(), &exts, ProfileKind::Rust)) {
            SignalResult::Findings { findings, .. } => {
                assert_eq!(findings.len(), 1);
                assert_eq!(findings[0].level, SignalLevel::Ticket);
                let t = findings[0].ticket.as_ref().unwrap();
                assert_eq!(t.domain, "core");
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn excludes_test_files() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("core/tests")).unwrap();
        let big = "x\n".repeat(650);
        fs::write(tmp.path().join("core/tests/huge_test.rs"), big).unwrap();
        let exts = vec!["rs".to_string()];
        match run(SignalId::OversizedFiles, &ctx(tmp.path(), &exts, ProfileKind::Rust)) {
            SignalResult::Skip(_) => {} // only a test file -> no product sources
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn debt_total_ticket() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        let body: String = (0..30).map(|i| format!("// TODO item {i}\n")).collect();
        fs::write(tmp.path().join("src/a.rs"), body).unwrap();
        let exts = vec!["rs".to_string()];
        match run(SignalId::DebtMarkers, &ctx(tmp.path(), &exts, ProfileKind::Rust)) {
            SignalResult::Findings { findings, .. } => {
                assert!(findings.iter().any(|f| f.level == SignalLevel::Ticket));
            }
            other => panic!("got {other:?}"),
        }
    }
}
