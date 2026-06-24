use crate::config::EffectiveConfig;
use crate::{config, context, output, process};
use clap::Args;

#[derive(Args, Debug)]
pub struct GenArgs {
    /// Run only the named generator.
    pub name: Option<String>,
    /// List configured generators and exit.
    #[arg(long)]
    pub list: bool,
}

pub fn run(args: GenArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let cfg = config::load(&root)?;

    if args.list {
        if cfg.codegen.is_empty() {
            output::note("no generators configured (add [[codegen]] entries to meta.toml)");
        } else {
            output::head("Configured generators");
            for g in &cfg.codegen {
                println!("  {}  {}  (trigger: {})", output::bold(&g.name), g.command, g.trigger);
            }
        }
        return Ok(0);
    }

    Ok(run_generators(&cfg, args.name.as_deref()))
}

/// Run codegen entries (optionally just `only`). A generator whose trigger file
/// is absent is skipped with a NOTE (exit 0), mirroring the bash behavior.
/// `meta build` calls this before building.
pub fn run_generators(cfg: &EffectiveConfig, only: Option<&str>) -> i32 {
    if cfg.codegen.is_empty() {
        return 0;
    }
    let mut worst = 0;
    for g in &cfg.codegen {
        if let Some(name) = only {
            if g.name != name {
                continue;
            }
        }
        if !cfg.root.join(&g.trigger).exists() {
            output::note(format!("gen: {} skipped (no trigger {})", g.name, g.trigger));
            continue;
        }
        let prog = process::program_of(&g.command).unwrap_or_default();
        if !process::which(&prog) {
            output::note(format!("gen: {} skipped ({prog} not on PATH)", g.name));
            continue;
        }
        output::head(format!("gen: {} → {}", g.name, g.command));
        let code = process::run_inherited(&g.command, &cfg.root);
        if code != 0 {
            output::err(format!("gen: {} failed (exit {code})", g.name));
            worst = code;
        }
    }
    worst
}
