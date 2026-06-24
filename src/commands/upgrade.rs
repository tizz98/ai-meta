use crate::config::migrate;
use crate::sync::{self, ChangeKind};
use crate::version::{Version, FRAMEWORK_VERSION};
use crate::{config, context, output};
use clap::Args;

#[derive(Args, Debug)]
pub struct UpgradeArgs {
    /// Show the diff of every artifact without writing.
    #[arg(long)]
    pub dry_run: bool,
    /// Target framework version to pin (defaults to this binary's version).
    #[arg(long)]
    pub to: Option<String>,
}

pub fn run(args: UpgradeArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let meta_path = config::config_path(&root);
    let meta_old = std::fs::read_to_string(&meta_path).map_err(|e| {
        anyhow::anyhow!("cannot read {} (run `meta init` first): {e}", meta_path.display())
    })?;

    let target = match &args.to {
        Some(t) => Version::parse(t)?.to_string(),
        None => FRAMEWORK_VERSION.to_string(),
    };

    // Migrate the user-owned config (schema bump, preserving comments/values),
    // then repin its framework_version to the target.
    let (migrated, steps) = migrate::migrate(&meta_old)?;
    let meta_new = migrate::set_framework_version(&migrated, &target)?;

    // Resolve config from the migrated text to drive artifact generation.
    let cfg = config::load_from_str(&root, &meta_new)?;
    let plan = sync::plan(&root, &cfg, &meta_old, &meta_new, &target, &cfg.sync_ignore);

    output::head(format!(
        "meta upgrade — {} → ai-meta v{target}",
        cfg.framework_version
    ));
    for s in &steps {
        output::info(format!("  migration: {s}"));
    }
    println!();

    let modified: Vec<_> = plan.modified().collect();
    if modified.is_empty() {
        output::ok("already up to date — nothing to change.");
        return Ok(0);
    }

    for c in &modified {
        let tag = match c.kind {
            ChangeKind::New => output::green("new"),
            ChangeKind::Modified => output::yellow("update"),
            ChangeKind::Unchanged => continue,
        };
        println!("  {tag}  {}", c.path);
        if args.dry_run {
            let d = sync::diff::unified(&c.old, &c.new);
            for line in d.lines() {
                println!("      {line}");
            }
        }
    }

    // Note any ignored files that would otherwise have changed.
    for c in plan.changes.iter().filter(|c| c.ignored && c.kind != ChangeKind::Unchanged) {
        output::note(format!("{} changed upstream but is in [sync] ignore — skipped", c.path));
    }

    println!();
    if args.dry_run {
        output::note(format!("dry run — {} file(s) would change. Re-run without --dry-run to apply.", modified.len()));
        return Ok(0);
    }

    let summary = sync::apply(&root, &plan)?;
    output::ok(format!(
        "updated {} file(s); {} unchanged{}.",
        summary.updated,
        summary.unchanged,
        if summary.ignored > 0 {
            format!("; {} ignored", summary.ignored)
        } else {
            String::new()
        }
    ));
    Ok(0)
}
