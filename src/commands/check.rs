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

pub fn run(_args: CheckArgs) -> anyhow::Result<i32> {
    super::not_yet("check", "P2")
}
