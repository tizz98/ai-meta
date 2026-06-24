use crate::config::EffectiveConfig;
use crate::{config, context, output, process, state};

pub fn run() -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let cfg = config::load(&root)?;

    // Auto-run any triggered generators first (mirrors bash `build`).
    let gen_code = super::gen::run_generators(&cfg, None);
    if gen_code != 0 {
        return Ok(gen_code);
    }

    match run_command(&cfg, cfg.build.as_deref(), "build") {
        Outcome::Skipped(reason) => {
            output::note(format!("build: {reason}"));
            Ok(0)
        }
        Outcome::Ran(code) => {
            let status = if code == 0 { "passed" } else { "failed" };
            let _ = state::record(&root, "build", status, cfg.build.as_deref().unwrap_or(""));
            Ok(code)
        }
    }
}

/// Shared command runner used by build/test: skips cleanly when there is no such
/// command for the profile or the toolchain is absent.
pub enum Outcome {
    Skipped(String),
    Ran(i32),
}

pub fn run_command(cfg: &EffectiveConfig, cmd: Option<&str>, label: &str) -> Outcome {
    let cmd = match cmd {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            return Outcome::Skipped(format!(
                "no {label} command for the {} profile",
                cfg.profile_kind.name()
            ))
        }
    };
    let prog = process::program_of(cmd).unwrap_or_default();
    if !process::which(&prog) {
        return Outcome::Skipped(format!("{prog} not on PATH — skipping {label}"));
    }
    output::head(format!("{label}: {cmd}"));
    Outcome::Ran(process::run_inherited(cmd, &cfg.root))
}
