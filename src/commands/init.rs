use clap::Args;

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Language profile (auto-detected from repo markers when omitted).
    #[arg(long)]
    pub profile: Option<String>,
    /// One-line project description (seeds wording for new projects).
    #[arg(long)]
    pub description: Option<String>,
    /// Show what would be written without touching the filesystem.
    #[arg(long)]
    pub dry_run: bool,
    /// Overwrite an existing .meta/meta.toml.
    #[arg(long)]
    pub force: bool,
    /// Skip the optional `claude` CLI wording enrichment.
    #[arg(long)]
    pub no_ai: bool,
}

pub fn run(_args: InitArgs) -> anyhow::Result<i32> {
    super::not_yet("init", "P4")
}
