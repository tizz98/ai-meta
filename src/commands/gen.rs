use clap::Args;

#[derive(Args, Debug)]
pub struct GenArgs {
    /// Run only the named generator.
    pub name: Option<String>,
    /// List configured generators and exit.
    #[arg(long)]
    pub list: bool,
}

pub fn run(_args: GenArgs) -> anyhow::Result<i32> {
    super::not_yet("gen", "P3")
}
