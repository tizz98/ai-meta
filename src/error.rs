//! Typed library errors. Command bodies use `anyhow` for context; the lower
//! layers (config, rules, sync, version) return these so callers can match.

use std::path::PathBuf;

/// Result alias for the library layers.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not inside a meta project: no .meta/ or .git found from {0}")]
    NotAProject(PathBuf),

    #[error("config error: {0}")]
    Config(String),

    #[error("unknown profile {0:?} (known: rust, typescript, python, generic)")]
    UnknownProfile(String),

    #[error("schema mismatch: repo config schema v{repo} is newer than this binary (v{binary}); update the pinned binary in .meta/version")]
    SchemaTooNew { repo: u32, binary: u32 },

    #[error("invalid version string {0:?}: expected semver X.Y.Z")]
    BadVersion(String),

    #[error("{0}")]
    Message(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn msg(s: impl Into<String>) -> Self {
        Error::Message(s.into())
    }
}
