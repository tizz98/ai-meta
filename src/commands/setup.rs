use clap::Args;

#[derive(Args, Debug)]
pub struct SetupArgs {
    /// Preview without making any GitHub changes.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(_args: SetupArgs) -> anyhow::Result<i32> {
    super::not_yet("setup", "P6")
}
