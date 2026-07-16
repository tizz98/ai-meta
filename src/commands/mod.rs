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
pub mod stats;
pub mod status;
pub mod tag;
pub mod task;
pub mod test;
pub mod upgrade;
pub mod wave;
