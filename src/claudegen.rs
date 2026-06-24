//! Optional `claude` CLI integration. When the CLI is available (and we're not
//! in CI / not disabled), we use it to tailor project wording — e.g. a one-line
//! description for `CLAUDE.md` — from the actual repo context. This is pure
//! enrichment over a working static baseline: if `claude` is absent, errors, or
//! is disabled, callers fall back to deterministic templates. Output is always
//! surfaced via the normal dry-run/diff path and never auto-committed.

use crate::process;
use std::path::Path;

/// Is the `claude` CLI usable for enrichment right now? False in CI / headless,
/// or when explicitly disabled, so generation stays deterministic there.
pub fn available(disabled: bool) -> bool {
    if disabled {
        return false;
    }
    if std::env::var_os("CI").is_some() || std::env::var_os("AI_META_NO_AI").is_some() {
        return false;
    }
    process::which("claude")
}

/// Ask `claude` for a one-line project description, seeded from the repo's
/// README + detected layout. Returns `None` on any failure (caller falls back).
pub fn describe_project(
    root: &Path,
    title: &str,
    profile: &str,
    domains: &[String],
) -> Option<String> {
    let readme = read_readme(root);
    let domains_line = if domains.is_empty() {
        String::new()
    } else {
        format!("\nTop-level areas: {}.", domains.join(", "))
    };
    let prompt = format!(
        "Write a single concise sentence (max 25 words, no preamble, no quotes) describing \
         what this software project does, for the top of its CLAUDE.md. \
         Project name: {title}. Language profile: {profile}.{domains_line}\n\n\
         README (may be empty):\n{readme}"
    );
    let out = process::run_captured(&format!("claude -p {}", shell_quote(&prompt)), root).ok()?;
    if out.status != 0 {
        return None;
    }
    let line = out.stdout.trim().lines().next()?.trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

fn read_readme(root: &Path) -> String {
    for name in ["README.md", "README", "readme.md"] {
        if let Ok(s) = std::fs::read_to_string(root.join(name)) {
            // Cap the context we feed the model.
            return s.chars().take(2000).collect();
        }
    }
    String::new()
}

/// Minimal POSIX single-quote shell quoting for passing a prompt as one arg.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_or_ci_means_unavailable() {
        assert!(!available(true));
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("a'b"), r"'a'\''b'");
        assert_eq!(shell_quote("plain"), "'plain'");
    }
}
