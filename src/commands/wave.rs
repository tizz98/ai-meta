use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct WaveArgs {
    #[command(subcommand)]
    pub sub: Option<WaveCmd>,
}

#[derive(Subcommand, Debug)]
pub enum WaveCmd {
    /// List wave epics.
    List,
    /// Show a wave epic's sub-issues.
    Show { epic: u64 },
    /// Plan a parallelizable wave from an epic's ready sub-issues.
    Ready { epic: u64 },
}

pub fn run(_args: WaveArgs) -> anyhow::Result<i32> {
    super::not_yet("wave", "P6")
}
