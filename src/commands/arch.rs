use crate::output;
use crate::rules::model::SignalLevel;
use crate::rules::{self, ArchReport};
use crate::{config, context, state};
use clap::Args;

#[derive(Args, Debug)]
pub struct ArchArgs {
    /// Exit non-zero on ticket-candidate signals (otherwise advisory).
    #[arg(long)]
    pub strict: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: ArchArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let cfg = config::load(&root)?;
    let report = rules::run_arch(&cfg);

    if args.json {
        print_json(&report);
    } else {
        render(&report);
    }

    let code = report.exit_code(args.strict);
    let _ = state::record(
        &root,
        "arch",
        if report.ticket > 0 { "tickets" } else { "clean" },
        &format!("{} watch / {} ticket", report.watch, report.ticket),
    );
    Ok(code)
}

fn render(report: &ArchReport) {
    output::head("meta arch — architecture review (advisory)");
    println!();
    for s in &report.signals {
        if let Some(reason) = &s.skipped {
            println!("  {}  {}  {}", output::dim("SKIP"), output::bold(&s.name), output::dim(reason));
            continue;
        }
        if let Some(clean) = &s.clean {
            println!("  {}  {}  {}", output::green("OK"), output::bold(&s.name), output::dim(clean));
            continue;
        }
        for f in &s.findings {
            let badge = match f.level {
                SignalLevel::Ticket => output::red("TICKET"),
                SignalLevel::Watch => output::yellow("WATCH"),
                _ => output::dim("OK"),
            };
            println!("  {badge}  {}  {}", output::bold(&s.name), f.summary);
            println!("          {}", output::dim(&f.detail));
        }
    }
    println!();
    output::info(format!(
        "{} clean · {} skipped · {} watch · {} ticket",
        report.clean, report.skip, report.watch, report.ticket
    ));
    if !report.tickets.is_empty() {
        println!();
        output::head("Recommended refactor tickets");
        for t in &report.tickets {
            println!(
                "  {} task new --type chore --domain {} {:?}",
                output::bold("meta"),
                t.domain,
                t.title
            );
        }
    }
}

fn print_json(report: &ArchReport) {
    let tickets: Vec<serde_json::Value> = report
        .tickets
        .iter()
        .map(|t| serde_json::json!({"title": t.title, "domain": t.domain, "body": t.body}))
        .collect();
    let doc = serde_json::json!({
        "clean": report.clean,
        "skip": report.skip,
        "watch": report.watch,
        "ticket": report.ticket,
        "tickets": tickets,
    });
    println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
}
