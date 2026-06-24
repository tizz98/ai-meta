//! GitHub token resolution: `GITHUB_TOKEN`/`GH_TOKEN` env first, then a
//! best-effort `gh auth token` (the one tolerated `gh` shell-out), else a clear
//! error. No token is ever logged.

use crate::error::{Error, Result};
use crate::process;

/// Resolve a token for API calls.
pub fn resolve_token() -> Result<String> {
    for var in ["GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(t) = std::env::var(var) {
            let t = t.trim().to_string();
            if !t.is_empty() {
                return Ok(t);
            }
        }
    }
    if process::which("gh") {
        if let Ok(out) = process::run_captured("gh auth token", &std::env::current_dir()?) {
            let t = out.stdout.trim().to_string();
            if out.status == 0 && !t.is_empty() {
                return Ok(t);
            }
        }
    }
    Err(Error::msg(
        "no GitHub token: set GITHUB_TOKEN (or GH_TOKEN), or run `gh auth login`.",
    ))
}
