//! One module per subcommand. Each exposes an `Args` struct (clap-derived) and
//! a `run` function returning the process exit code.

pub mod arch;
pub mod build;
pub mod check;
pub mod ci;
pub mod gen;
pub mod init;
pub mod milestone;
pub mod setup;
pub mod status;
pub mod tag;
pub mod task;
pub mod test;
pub mod upgrade;
pub mod wave;

/// Shared stub for commands not yet wired up, so the CLI surface is complete
/// while phases land. Prints a clear NOTE and succeeds.
pub(crate) fn not_yet(name: &str, phase: &str) -> anyhow::Result<i32> {
    crate::output::note(format!("`meta {name}` is not implemented yet (lands in {phase})."));
    Ok(0)
}
