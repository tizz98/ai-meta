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

pub fn run(_args: ArchArgs) -> anyhow::Result<i32> {
    super::not_yet("arch", "P2")
}
