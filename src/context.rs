//! Execution context: locate the repo root and (later) hold the loaded config
//! and resolved profile. For now it resolves the root so every command shares
//! the same anchor.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

/// Resolve the repo root by walking up from `start` looking for, in order of
/// preference, a `.meta/meta.toml`, a `.meta/` dir, or a `.git/` dir.
pub fn find_root(start: &Path) -> Result<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join(".meta").join("meta.toml").is_file()
            || d.join(".meta").is_dir()
            || d.join(".git").exists()
        {
            return Ok(d.to_path_buf());
        }
        dir = d.parent();
    }
    Err(Error::NotAProject(start.to_path_buf()))
}

/// Resolve the root for commands that operate on an existing project; errors
/// with a clear message when run outside one.
pub fn require_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    find_root(&cwd)
}

/// Resolve a root for scaffolding (`init`): prefer a `.git` root so `init` lands
/// at the repo top, else fall back to the current directory.
pub fn scaffold_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let mut dir = Some(cwd.as_path());
    while let Some(d) = dir {
        if d.join(".git").exists() {
            return Ok(d.to_path_buf());
        }
        dir = d.parent();
    }
    Ok(cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_meta_toml_root() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(tmp.path().join(".meta")).unwrap();
        fs::write(tmp.path().join(".meta").join("meta.toml"), "x").unwrap();
        assert_eq!(find_root(&nested).unwrap(), tmp.path());
    }

    #[test]
    fn finds_git_root() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("sub");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        assert_eq!(find_root(&nested).unwrap(), tmp.path());
    }

    #[test]
    fn errors_outside_project() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(find_root(tmp.path()).is_err());
    }
}
