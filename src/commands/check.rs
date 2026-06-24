use crate::output;
use crate::rules::model::Severity;
use crate::rules::{self, CheckReport};
use crate::{config, context, state};
use clap::Args;

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Treat warnings as failures.
    #[arg(long)]
    pub strict: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: CheckArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let cfg = config::load(&root)?;
    let report = rules::run_checks(&cfg);

    if args.json {
        print_json(&report);
    } else {
        render(&report);
    }

    let code = report.exit_code(args.strict);
    let status = if code == 0 { "passed" } else { "failed" };
    let _ = state::record(
        &root,
        "check",
        status,
        &format!(
            "{} pass / {} warn / {} fail",
            report.pass, report.warn, report.fail
        ),
    );
    Ok(code)
}

fn render(report: &CheckReport) {
    output::head("meta check — codified standards");
    println!();
    for e in &report.entries {
        let badge = badge(e.verdict);
        println!(
            "  {badge}  {}  {}",
            output::bold(&e.name),
            output::dim(&e.summary)
        );
        for h in &e.hits {
            println!("        {}", output::dim(h));
        }
    }
    println!();
    output::info(format!(
        "{} passed · {} skipped · {} warned · {} failed",
        report.pass, report.skip, report.warn, report.fail
    ));
}

fn badge(s: Severity) -> String {
    match s {
        Severity::Pass => output::green("PASS"),
        Severity::Skip => output::dim("SKIP"),
        Severity::Warn => output::yellow("WARN"),
        Severity::Fail => output::red("FAIL"),
    }
}

fn print_json(report: &CheckReport) {
    let entries: Vec<serde_json::Value> = report
        .entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "name": e.name,
                "verdict": e.verdict.badge().to_lowercase(),
                "summary": e.summary,
                "hits": e.hits,
            })
        })
        .collect();
    let doc = serde_json::json!({
        "pass": report.pass,
        "skip": report.skip,
        "warn": report.warn,
        "fail": report.fail,
        "entries": entries,
    });
    println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
}
