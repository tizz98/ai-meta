use crate::github::Github;
use crate::{context, output};
use clap::{Args, Subcommand};
use serde_json::Value;

#[derive(Args, Debug)]
pub struct MilestoneArgs {
    #[command(subcommand)]
    pub sub: Option<MilestoneCmd>,
}

#[derive(Subcommand, Debug)]
pub enum MilestoneCmd {
    /// List milestones with completion %.
    List,
    /// Show the issues in a milestone.
    Show { title: String },
}

pub fn run(args: MilestoneArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let gh = Github::connect(&root)?;

    match args.sub.unwrap_or(MilestoneCmd::List) {
        MilestoneCmd::List => {
            let ms = gh.list_milestones()?;
            output::head(format!("Milestones ({})", gh.repo.nwo()));
            if ms.is_empty() {
                output::info("  (none — run `meta setup`)");
            }
            for m in &ms {
                let open = uint(m, "open_issues");
                let closed = uint(m, "closed_issues");
                let total = open + closed;
                let pct = (closed * 100).checked_div(total).unwrap_or(0);
                println!(
                    "  {:>3}%  {}  ({}/{} done)",
                    pct,
                    m.get("title").and_then(|t| t.as_str()).unwrap_or(""),
                    closed,
                    total
                );
            }
        }
        MilestoneCmd::Show { title } => {
            // Find the milestone, then list its issues (open + closed).
            let open = gh.list_issues("all", None)?;
            output::head(format!("Milestone: {title}"));
            let mut any = false;
            for i in &open {
                if i.pointer("/milestone/title").and_then(|t| t.as_str()) == Some(title.as_str()) {
                    any = true;
                    println!(
                        "  #{:<4} [{}] {}",
                        i.get("number").and_then(|n| n.as_u64()).unwrap_or(0),
                        i.get("state").and_then(|s| s.as_str()).unwrap_or("?"),
                        i.get("title").and_then(|t| t.as_str()).unwrap_or("")
                    );
                }
            }
            if !any {
                output::info("  (no issues)");
            }
        }
    }
    Ok(0)
}

fn uint(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}
