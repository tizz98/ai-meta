use crate::config::EffectiveConfig;
use crate::profile::CoverageTool;
use crate::rules::{self, coverage};
use crate::{config, context, output, process, state};
use clap::Args;

#[derive(Args, Debug)]
pub struct CiArgs {
    /// PR number to post a collapsed result comment to (posting lands in P6).
    pub pr: Option<u64>,
    /// Gate on the architecture review too.
    #[arg(long)]
    pub arch_strict: bool,
    /// Skip the architecture review.
    #[arg(long)]
    pub no_arch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateOutcome {
    Pass,
    Fail,
    Skip,
}

struct Gate {
    name: String,
    hard: bool,
    outcome: GateOutcome,
    detail: String,
}

pub fn run(args: CiArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let cfg = config::load(&root)?;
    let mut gates: Vec<Gate> = Vec::new();

    output::head(format!("meta ci — local merge gate for {}", cfg.title));
    println!();

    // Gate 1: codified standards (in-process, strict — mirrors `check --strict`).
    let check = rules::run_checks(&cfg);
    gates.push(Gate {
        name: "standards".into(),
        hard: true,
        outcome: if check.exit_code(true) == 0 { GateOutcome::Pass } else { GateOutcome::Fail },
        detail: format!("{} pass / {} warn / {} fail", check.pass, check.warn, check.fail),
    });

    // Gates 2-5: format, lint, typecheck, test.
    gates.push(cmd_gate(&cfg, "format", cfg.fmt.as_deref(), true));
    gates.push(cmd_gate(&cfg, "lint", cfg.lint.as_deref(), true));
    if cfg.typecheck.is_some() {
        gates.push(cmd_gate(&cfg, "typecheck", cfg.typecheck.as_deref(), true));
    }
    gates.push(cmd_gate(&cfg, "test", cfg.test.as_deref(), true));

    // Gate 6: coverage.
    gates.push(coverage_gate(&cfg));

    // Gate 7: build.
    gates.push(cmd_gate(&cfg, "build", cfg.build.as_deref(), true));

    // Gates 8+: configured extra gates (e.g. SDK builds).
    for eg in &cfg.ci_extra_gates {
        gates.push(extra_gate(&cfg, eg));
    }

    // Advisory architecture review.
    if !args.no_arch {
        let arch = rules::run_arch(&cfg);
        let outcome = if arch.ticket > 0 && args.arch_strict {
            GateOutcome::Fail
        } else if arch.ticket > 0 {
            GateOutcome::Skip // advisory: surfaced, not blocking
        } else {
            GateOutcome::Pass
        };
        gates.push(Gate {
            name: "architecture (advisory)".into(),
            hard: args.arch_strict,
            outcome,
            detail: format!("{} watch / {} ticket", arch.watch, arch.ticket),
        });
    }

    println!();
    render(&gates);
    let body = comment_body(&cfg, &gates);

    let failed = gates.iter().any(|g| g.hard && g.outcome == GateOutcome::Fail);
    let _ = state::record(
        &root,
        "ci",
        if failed { "failed" } else { "passed" },
        &format!("{} gates", gates.len()),
    );

    if let Some(pr) = args.pr {
        output::note(format!(
            "posting the collapsed result to PR #{pr} requires the GitHub layer (P6); comment body assembled below."
        ));
        println!("\n{body}");
    }

    Ok(if failed { 1 } else { 0 })
}

fn cmd_gate(cfg: &EffectiveConfig, name: &str, cmd: Option<&str>, hard: bool) -> Gate {
    let cmd = match cmd {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            return Gate {
                name: name.into(),
                hard,
                outcome: GateOutcome::Skip,
                detail: "no command for this profile".into(),
            }
        }
    };
    let prog = process::program_of(cmd).unwrap_or_default();
    if !process::which(&prog) {
        return Gate {
            name: name.into(),
            hard,
            outcome: GateOutcome::Skip,
            detail: format!("{prog} not on PATH"),
        };
    }
    output::head(format!("{name}: {cmd}"));
    let code = process::run_inherited(cmd, &cfg.root);
    Gate {
        name: name.into(),
        hard,
        outcome: if code == 0 { GateOutcome::Pass } else { GateOutcome::Fail },
        detail: cmd.to_string(),
    }
}

fn extra_gate(cfg: &EffectiveConfig, eg: &crate::config::schema::ExtraGate) -> Gate {
    if let Some(dir) = &eg.when_dir {
        if !cfg.root.join(dir).exists() {
            return Gate {
                name: eg.name.clone(),
                hard: eg.hard,
                outcome: GateOutcome::Skip,
                detail: format!("{dir}/ absent"),
            };
        }
    }
    let cwd = eg.cwd.as_ref().map(|c| cfg.root.join(c)).unwrap_or_else(|| cfg.root.clone());
    let prog = process::program_of(&eg.command).unwrap_or_default();
    if !process::which(&prog) {
        return Gate {
            name: eg.name.clone(),
            hard: eg.hard,
            outcome: GateOutcome::Skip,
            detail: format!("{prog} not on PATH"),
        };
    }
    output::head(format!("{}: {}", eg.name, eg.command));
    let code = process::run_inherited(&eg.command, &cwd);
    Gate {
        name: eg.name.clone(),
        hard: eg.hard,
        outcome: if code == 0 { GateOutcome::Pass } else { GateOutcome::Fail },
        detail: eg.command.clone(),
    }
}

fn coverage_gate(cfg: &EffectiveConfig) -> Gate {
    let gated = cfg.coverage_min > 0;
    let cmd = match cfg.coverage.as_deref() {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            return Gate {
                name: "coverage".into(),
                hard: gated,
                outcome: GateOutcome::Skip,
                detail: "no coverage command".into(),
            }
        }
    };
    let prog = process::program_of(cmd).unwrap_or_default();
    if !process::which(&prog) {
        return Gate {
            name: "coverage".into(),
            hard: gated,
            outcome: GateOutcome::Skip,
            detail: format!("{prog} not on PATH"),
        };
    }
    output::head(format!("coverage: {cmd}"));
    let code = process::run_inherited(cmd, &cfg.root);
    if code != 0 {
        return Gate {
            name: "coverage".into(),
            hard: gated,
            outcome: if gated { GateOutcome::Fail } else { GateOutcome::Skip },
            detail: format!("coverage command exited {code}"),
        };
    }
    let pct = determine_coverage(cfg);
    match (gated, pct) {
        (false, Some(p)) => Gate {
            name: "coverage".into(),
            hard: false,
            outcome: GateOutcome::Pass,
            detail: format!("{p:.1}% (report-only)"),
        },
        (false, None) => Gate {
            name: "coverage".into(),
            hard: false,
            outcome: GateOutcome::Skip,
            detail: "report-only; percent unavailable".into(),
        },
        (true, Some(p)) if p + f64::EPSILON >= cfg.coverage_min as f64 => Gate {
            name: "coverage".into(),
            hard: true,
            outcome: GateOutcome::Pass,
            detail: format!("{p:.1}% ≥ {}%", cfg.coverage_min),
        },
        (true, Some(p)) => Gate {
            name: "coverage".into(),
            hard: true,
            outcome: GateOutcome::Fail,
            detail: format!("{p:.1}% < {}%", cfg.coverage_min),
        },
        (true, None) => Gate {
            name: "coverage".into(),
            hard: true,
            outcome: GateOutcome::Fail, // fail closed
            detail: format!("coverage percent unavailable (gate ≥ {}%)", cfg.coverage_min),
        },
    }
}

/// Best-effort coverage percentage for the configured tool.
fn determine_coverage(cfg: &EffectiveConfig) -> Option<f64> {
    match cfg.coverage_tool {
        CoverageTool::Vitest => {
            let path = cfg.coverage_summary.as_deref()?;
            let json = std::fs::read_to_string(cfg.root.join(path)).ok()?;
            coverage::parse_vitest(&json)
        }
        CoverageTool::CargoLlvmCov => {
            let out =
                process::run_captured("cargo llvm-cov report --summary-only --json", &cfg.root).ok()?;
            if out.status != 0 {
                return None;
            }
            coverage::parse_llvm_cov(&out.stdout)
        }
        CoverageTool::PytestCov | CoverageTool::None => None,
    }
}

fn render(gates: &[Gate]) {
    output::head("Gate summary");
    for g in gates {
        let badge = match g.outcome {
            GateOutcome::Pass => output::green("✅ pass"),
            GateOutcome::Fail => output::red("❌ fail"),
            GateOutcome::Skip => output::dim("⤵️  skip"),
        };
        let hard = if g.hard { "" } else { " (advisory)" };
        println!("  {badge}  {}{}  {}", output::bold(&g.name), hard, output::dim(&g.detail));
    }
}

/// Assemble the collapsed PR-comment body (posted by the GitHub layer in P6).
fn comment_body(cfg: &EffectiveConfig, gates: &[Gate]) -> String {
    let failed = gates.iter().any(|g| g.hard && g.outcome == GateOutcome::Fail);
    let header = if failed { "❌ Local CI failed" } else { "✅ Local CI passed" };
    let mut s = String::new();
    s.push_str(&format!("<!-- {}-local-ci -->\n", cfg.title));
    s.push_str(&format!("**{header}**\n\n<details><summary>Gate details</summary>\n\n"));
    s.push_str("| Gate | Result | Detail |\n|---|---|---|\n");
    for g in gates {
        let r = match g.outcome {
            GateOutcome::Pass => "✅ pass",
            GateOutcome::Fail => "❌ fail",
            GateOutcome::Skip => "⤵️ skip",
        };
        s.push_str(&format!("| {} | {} | {} |\n", g.name, r, g.detail));
    }
    s.push_str("\n</details>\n");
    s
}
