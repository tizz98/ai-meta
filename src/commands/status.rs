use crate::github::Github;
use crate::{config, context, output, state};

pub fn run() -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let cfg = config::load(&root)?;

    output::head(format!("{} — status", cfg.title));
    println!("  profile:  {}", cfg.profile_kind.name());
    println!("  pinned:   ai-meta v{}", cfg.framework_version);
    if let Some(branch) = git_branch(&root) {
        println!("  branch:   {branch}");
    }

    output::head("\nLast runs");
    for key in ["build", "test", "check", "ci", "arch"] {
        println!("  {:<7} {}", key, state::describe(&root, key));
    }

    // GitHub task counts (best-effort — needs a token).
    output::head("\nTasks");
    match Github::connect(&root) {
        Ok(gh) => match gh.list_issues("open", None) {
            Ok(issues) => {
                let mut by_status: std::collections::BTreeMap<String, usize> = Default::default();
                for i in &issues {
                    let s = i
                        .get("labels")
                        .and_then(|l| l.as_array())
                        .and_then(|arr| {
                            arr.iter()
                                .filter_map(|l| l.get("name").and_then(|n| n.as_str()))
                                .find(|n| n.starts_with("status:"))
                                .map(|n| n.trim_start_matches("status:").to_string())
                        })
                        .unwrap_or_else(|| "unlabeled".into());
                    *by_status.entry(s).or_default() += 1;
                }
                println!("  {} open", issues.len());
                for (s, n) in by_status {
                    println!("    {s}: {n}");
                }
            }
            Err(e) => output::note(format!("tasks unavailable: {e}")),
        },
        Err(e) => output::note(format!("GitHub not connected ({e})")),
    }
    Ok(0)
}

fn git_branch(root: &std::path::Path) -> Option<String> {
    let out = crate::process::run_captured("git rev-parse --abbrev-ref HEAD", root).ok()?;
    if out.status == 0 {
        Some(out.stdout.trim().to_string())
    } else {
        None
    }
}
