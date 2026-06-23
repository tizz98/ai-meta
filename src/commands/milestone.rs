use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct MilestoneArgs {
    #[command(subcommand)]
    pub sub: Option<MilestoneCmd>,
}

#[derive(Subcommand, Debug)]
pub enum MilestoneCmd {
    /// List milestones with completion %.
    List,
    /// Show the issues in a milestone.
    Show { title: String },
}

pub fn run(_args: MilestoneArgs) -> anyhow::Result<i32> {
    super::not_yet("milestone", "P6")
}
