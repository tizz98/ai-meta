use clap::Args;

#[derive(Args, Debug)]
pub struct TestArgs {
    /// Run with coverage.
    #[arg(long)]
    pub coverage: bool,
}

pub fn run(_args: TestArgs) -> anyhow::Result<i32> {
    super::not_yet("test", "P3")
}
