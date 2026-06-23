use clap::Args;

#[derive(Args, Debug)]
pub struct CiArgs {
    /// PR number to post a collapsed result comment to.
    pub pr: Option<u64>,
    /// Gate on the architecture review too.
    #[arg(long)]
    pub arch_strict: bool,
    /// Skip the architecture review.
    #[arg(long)]
    pub no_arch: bool,
}

pub fn run(_args: CiArgs) -> anyhow::Result<i32> {
    super::not_yet("ci", "P3")
}
