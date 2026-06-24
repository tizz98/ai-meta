use crate::config::EffectiveConfig;
use crate::github::{projects, Github};
use crate::{config, context, output};
use clap::Args;

#[derive(Args, Debug)]
pub struct SetupArgs {
    /// Preview without making any GitHub changes.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: SetupArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let cfg = config::load(&root)?;

    let labels = resolve_labels(&cfg);

    output::head(format!("meta setup — GitHub structure for {}", cfg.title));
    if args.dry_run {
        output::head("\nLabels");
        for (name, color, _) in &labels {
            println!("  #{color}  {name}");
        }
        output::head("\nMilestones");
        for (title, _) in &cfg.milestones {
            println!("  {title}");
        }
        output::head("\nProject board");
        println!("  {}", cfg.board);
        output::note("dry run — no GitHub changes made.");
        return Ok(0);
    }

    let gh = Github::connect(&root)?;
    output::info(format!("  repo: {}", gh.repo.nwo()));

    output::head("Labels");
    for (name, color, desc) in &labels {
        gh.ensure_label(name, color, desc)?;
        output::ok(name);
    }

    output::head("Milestones");
    for (title, desc) in &cfg.milestones {
        gh.ensure_milestone(title, desc)?;
        output::ok(title);
    }

    output::head("Project board");
    match projects::ensure_board(&gh, &gh.repo.owner, &cfg.board) {
        Ok(_) => output::ok(&cfg.board),
        Err(e) => output::warn(format!(
            "board '{}' not created ({e}) — needs a token with the 'project' scope; continuing.",
            cfg.board
        )),
    }

    output::ok("setup complete.");
    Ok(0)
}

/// The full label set: status/type from the palette, domain:* for each domain,
/// plus any extra palette entries — each with a color + description.
fn resolve_labels(cfg: &EffectiveConfig) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    let desc_of = |name: &str| {
        cfg.label_descriptions
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let color_of = |name: &str| {
        cfg.label_colors
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "ededed".to_string())
    };

    let mut push = |name: String, color: String, desc: String| {
        if !out.iter().any(|(n, _, _)| *n == name) {
            out.push((name, color, desc));
        }
    };

    for s in &cfg.statuses {
        let n = format!("status:{s}");
        push(n.clone(), color_of(&n), desc_of(&n));
    }
    for t in &cfg.types {
        let n = format!("type:{t}");
        push(n.clone(), color_of(&n), desc_of(&n));
    }
    for d in &cfg.domains {
        let n = format!("domain:{d}");
        push(n.clone(), color_of(&n), desc_of(&n));
    }
    // Any extra palette entries (e.g. specials like semantics:delivery).
    for (name, color) in &cfg.label_colors {
        push(name.clone(), color.clone(), desc_of(name));
    }
    out
}
