//! The clap command tree and top-level dispatch. Mirrors the bash `meta`
//! entrypoint: every subcommand the framework supports, plus the new `init` and
//! `upgrade`. Each arm delegates to a function in [`crate::commands`].

use crate::commands;
use crate::output;
use crate::version::{FRAMEWORK_VERSION, SCHEMA_VERSION};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "meta",
    version = FRAMEWORK_VERSION,
    about = "Project state, task tracking, codified standards & templating — one versioned CLI.",
    propagate_version = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Disable colored output.
    #[arg(long, global = true)]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Scaffold .meta/, the ./meta shim, GH workflows, and agent docs for a project.
    Init(commands::init::InitArgs),
    /// Project state at a glance: branch, milestone progress, task counts, last runs.
    Status,
    /// Track work as GitHub issues (list/show/new/start/block/done/comment).
    Task(commands::task::TaskArgs),
    /// Show delivery-milestone progress and contents.
    Milestone(commands::milestone::MilestoneArgs),
    /// Plan a parallelizable wave of sub-issues for subagent dispatch (read-only).
    Wave(commands::wave::WaveArgs),
    /// Run code generators codified in meta.toml (none by default).
    Gen(commands::gen::GenArgs),
    /// Build the project (profile/config build command).
    Build,
    /// Run the project's tests.
    Test(commands::test::TestArgs),
    /// Enforce codified standards (guards).
    Check(commands::check::CheckArgs),
    /// Run the local-CI suite (mirrors GH Actions) and optionally post a PR comment.
    Ci(commands::ci::CiArgs),
    /// Architecture review: flag tech debt + recommend refactor tickets (advisory).
    Arch(commands::arch::ArchArgs),
    /// Idempotently create GitHub labels, milestones, and the Project board from config.
    Setup(commands::setup::SetupArgs),
    /// Repo analytics: commit counts by author, lines of code by language.
    Stats(commands::stats::StatsArgs),
    /// Cut a release: bump the version, commit, tag, and push.
    Tag(commands::tag::TagArgs),
    /// Update generated artifacts + migrate meta.toml to a newer framework version.
    Upgrade(commands::upgrade::UpgradeArgs),
    /// Print the framework + schema version.
    Version,
}

/// Parse args and run. Returns the process exit code.
pub fn run() -> i32 {
    let cli = Cli::parse();
    output::init_color();
    if cli.no_color {
        output::set_color(false);
    }

    let result: anyhow::Result<i32> = match cli.command {
        Command::Init(args) => commands::init::run(args),
        Command::Status => commands::status::run(),
        Command::Task(args) => commands::task::run(args),
        Command::Milestone(args) => commands::milestone::run(args),
        Command::Wave(args) => commands::wave::run(args),
        Command::Gen(args) => commands::gen::run(args),
        Command::Build => commands::build::run(),
        Command::Test(args) => commands::test::run(args),
        Command::Check(args) => commands::check::run(args),
        Command::Ci(args) => commands::ci::run(args),
        Command::Arch(args) => commands::arch::run(args),
        Command::Setup(args) => commands::setup::run(args),
        Command::Stats(args) => commands::stats::run(args),
        Command::Tag(args) => commands::tag::run(args),
        Command::Upgrade(args) => commands::upgrade::run(args),
        Command::Version => {
            output::info(format!(
                "meta v{FRAMEWORK_VERSION}  (config schema v{SCHEMA_VERSION})"
            ));
            Ok(0)
        }
    };

    match result {
        Ok(code) => code,
        Err(e) => {
            output::err(format!("{e:#}"));
            1
        }
    }
}
