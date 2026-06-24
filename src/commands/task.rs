use crate::github::Github;
use crate::{config, context, output};
use clap::{Args, Subcommand};
use serde_json::Value;

#[derive(Args, Debug)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub sub: Option<TaskCmd>,
}

#[derive(Subcommand, Debug)]
pub enum TaskCmd {
    /// List open tasks (optionally filtered by a status:* value).
    List {
        #[arg(long)]
        status: Option<String>,
    },
    /// Show one task by number.
    Show { number: u64 },
    /// Create a new task.
    New {
        title: String,
        #[arg(long, default_value = "feature")]
        r#type: String,
        #[arg(long)]
        domain: Option<String>,
        #[arg(long)]
        milestone: Option<String>,
        #[arg(long, default_value = "")]
        body: String,
    },
    /// Move a task to in-progress.
    Start { number: u64 },
    /// Mark a task blocked.
    Block { number: u64 },
    /// Mark a task done (sets status:done and closes the issue).
    Done { number: u64 },
    /// Comment on a task.
    Comment { number: u64, body: String },
}

pub fn run(args: TaskArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let cfg = config::load(&root)?;
    let gh = Github::connect(&root)?;

    match args.sub.unwrap_or(TaskCmd::List { status: None }) {
        TaskCmd::List { status } => {
            let label = status.map(|s| format!("status:{s}"));
            let issues = gh.list_issues("open", label.as_deref())?;
            output::head(format!("Open tasks ({})", gh.repo.nwo()));
            if issues.is_empty() {
                output::info("  (none)");
            }
            for i in &issues {
                println!(
                    "  #{:<4} {}  {}",
                    num(i),
                    output::dim(&status_of(i)),
                    title(i)
                );
            }
        }
        TaskCmd::Show { number } => {
            let i = gh.get_issue(number)?;
            output::head(format!("#{} {}", number, title(&i)));
            println!("  state:  {}", i.get("state").and_then(|s| s.as_str()).unwrap_or("?"));
            println!("  labels: {}", labels_of(&i).join(", "));
            if let Some(m) = i.pointer("/milestone/title").and_then(|t| t.as_str()) {
                println!("  milestone: {m}");
            }
            if let Some(b) = i.get("body").and_then(|b| b.as_str()) {
                if !b.is_empty() {
                    println!("\n{b}");
                }
            }
        }
        TaskCmd::New { title, r#type, domain, milestone, body } => {
            let default_status = cfg.statuses.first().cloned().unwrap_or_else(|| "todo".into());
            let mut labels = vec![format!("status:{default_status}"), format!("type:{}", r#type)];
            if let Some(d) = domain {
                labels.push(format!("domain:{d}"));
            }
            let ms = match milestone {
                Some(t) => Some(resolve_milestone(&gh, &t)?),
                None => None,
            };
            let n = gh.create_issue(&title, &body, &labels, ms)?;
            output::ok(format!("created #{n}: {title}"));
        }
        TaskCmd::Start { number } => set_status(&gh, number, "in-progress")?,
        TaskCmd::Block { number } => set_status(&gh, number, "blocked")?,
        TaskCmd::Done { number } => {
            gh.set_status(number, "done")?;
            gh.set_state(number, "closed")?;
            output::ok(format!("#{number} → done (closed)"));
        }
        TaskCmd::Comment { number, body } => {
            gh.comment(number, &body)?;
            output::ok(format!("commented on #{number}"));
        }
    }
    Ok(0)
}

fn set_status(gh: &Github, number: u64, status: &str) -> anyhow::Result<()> {
    gh.set_status(number, status)?;
    output::ok(format!("#{number} → {status}"));
    Ok(())
}

fn resolve_milestone(gh: &Github, title: &str) -> anyhow::Result<u64> {
    let ms = gh.list_milestones()?;
    ms.into_iter()
        .find(|m| m.get("title").and_then(|t| t.as_str()) == Some(title))
        .and_then(|m| m.get("number").and_then(|n| n.as_u64()))
        .ok_or_else(|| anyhow::anyhow!("no milestone titled {title:?} (run `meta setup`)"))
}

fn num(i: &Value) -> u64 {
    i.get("number").and_then(|n| n.as_u64()).unwrap_or(0)
}
fn title(i: &Value) -> String {
    i.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string()
}
fn labels_of(i: &Value) -> Vec<String> {
    i.get("labels")
        .and_then(|l| l.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|l| l.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}
fn status_of(i: &Value) -> String {
    labels_of(i)
        .into_iter()
        .find(|l| l.starts_with("status:"))
        .unwrap_or_else(|| "status:?".into())
}
