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

pub fn run(_args: UpgradeArgs) -> anyhow::Result<i32> {
    super::not_yet("upgrade", "P5")
}
