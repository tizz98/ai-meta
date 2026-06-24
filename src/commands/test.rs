use crate::commands::build::{run_command, Outcome};
use crate::{config, context, output, state};
use clap::Args;

#[derive(Args, Debug)]
pub struct TestArgs {
    /// Run with coverage (uses the configured coverage command).
    #[arg(long)]
    pub coverage: bool,
}

pub fn run(args: TestArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let cfg = config::load(&root)?;

    let (cmd, label) = if args.coverage {
        (cfg.coverage.as_deref(), "coverage")
    } else {
        (cfg.test.as_deref(), "test")
    };

    match run_command(&cfg, cmd, label) {
        Outcome::Skipped(reason) => {
            output::note(format!("{label}: {reason}"));
            Ok(0)
        }
        Outcome::Ran(code) => {
            let status = if code == 0 { "passed" } else { "failed" };
            let _ = state::record(&root, "test", status, cmd.unwrap_or(""));
            Ok(code)
        }
    }
}
