use clap::Args;

#[derive(Args, Debug)]
pub struct TagArgs {
    /// Release level (major|minor|patch) or explicit vX.Y.Z. Defaults to minor.
    pub level: Option<String>,
    /// Print what would change without editing, committing, tagging, or pushing.
    #[arg(long)]
    pub dry_run: bool,
    /// Proceed even when not on the configured release branch.
    #[arg(long)]
    pub allow_branch: bool,
}

pub fn run(_args: TagArgs) -> anyhow::Result<i32> {
    super::not_yet("tag", "P7")
}
