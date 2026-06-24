use crate::github::Github;
use crate::{context, output};
use clap::{Args, Subcommand};
use serde_json::Value;

#[derive(Args, Debug)]
pub struct WaveArgs {
    #[command(subcommand)]
    pub sub: Option<WaveCmd>,
}

#[derive(Subcommand, Debug)]
pub enum WaveCmd {
    /// List epics (issues labeled type:epic).
    List,
    /// Show an epic's sub-issues.
    Show { epic: u64 },
    /// Plan the ready (status:todo, unblocked) sub-issues of an epic.
    Ready { epic: u64 },
}

pub fn run(args: WaveArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let gh = Github::connect(&root)?;

    match args.sub.unwrap_or(WaveCmd::List) {
        WaveCmd::List => {
            let epics = gh.list_issues("open", Some("type:epic"))?;
            output::head("Epics");
            if epics.is_empty() {
                output::info("  (none — label an issue 'type:epic')");
            }
            for e in &epics {
                println!("  #{:<4} {}", num(e), title(e));
            }
        }
        WaveCmd::Show { epic } => {
            let subs = gh.sub_issues(epic)?;
            print_subs(&format!("Sub-issues of #{epic}"), &subs);
        }
        WaveCmd::Ready { epic } => {
            let subs = gh.sub_issues(epic)?;
            let ready: Vec<Value> = subs
                .into_iter()
                .filter(|s| status_of(s) == "todo" && state_of(s) == "open")
                .collect();
            print_subs(&format!("Ready wave for #{epic} (status:todo, open)"), &ready);
            output::info(format!("\n{} task(s) ready to dispatch.", ready.len()));
        }
    }
    Ok(0)
}

fn print_subs(header: &str, subs: &[Value]) {
    output::head(header);
    if subs.is_empty() {
        output::info("  (none)");
    }
    for s in subs {
        println!("  #{:<4} [{}] {}", num(s), status_of(s), title(s));
    }
}

fn num(i: &Value) -> u64 {
    i.get("number").and_then(|n| n.as_u64()).unwrap_or(0)
}
fn title(i: &Value) -> String {
    i.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string()
}
fn state_of(i: &Value) -> String {
    i.get("state").and_then(|s| s.as_str()).unwrap_or("open").to_string()
}
fn status_of(i: &Value) -> String {
    i.get("labels")
        .and_then(|l| l.as_array())
        .and_then(|arr| {
            arr.iter()
                .filter_map(|l| l.get("name").and_then(|n| n.as_str()))
                .find(|n| n.starts_with("status:"))
                .map(|n| n.trim_start_matches("status:").to_string())
        })
        .unwrap_or_else(|| "?".into())
}
