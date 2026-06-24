use crate::config::EffectiveConfig;
use crate::version::{BumpLevel, Version};
use crate::{config, context, output, process, state};
use clap::Args;
use regex::Regex;
use std::path::Path;

#[derive(Args, Debug)]
pub struct TagArgs {
    /// Release level (major|minor|patch) or explicit vX.Y.Z. Defaults to the
    /// configured default_bump (minor).
    pub level: Option<String>,
    /// Print what would change without editing, committing, tagging, or pushing.
    #[arg(long)]
    pub dry_run: bool,
    /// Proceed even when not on the configured release branch.
    #[arg(long)]
    pub allow_branch: bool,
}

pub fn run(args: TagArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    let cfg = config::load(&root)?;

    let cur = read_current(&root, &cfg).ok_or_else(|| {
        anyhow::anyhow!("could not read a semver from the configured version locations")
    })?;

    let level = args
        .level
        .clone()
        .unwrap_or_else(|| cfg.default_bump.clone());
    let next = match BumpLevel::parse(&level) {
        Some(b) => cur.bump(b),
        None => Version::parse(&level).map_err(|_| {
            anyhow::anyhow!("invalid level {level:?} — expected major|minor|patch or vX.Y.Z")
        })?,
    };

    let prefix = tag_prefix(&root, &cfg);
    let tagname = format!("{prefix}{next}");
    let branch = git_branch(&root).unwrap_or_default();

    output::head(format!("Release: {cur} → {next}  (tag: {tagname})"));

    let planned = plan_changes(&root, &cfg, &cur, &next);

    if args.dry_run {
        output::head("\nFiles that would change");
        for (path, line) in &planned {
            println!("  {}: {}", output::bold(path), output::dim(line));
        }
        if next == cur {
            output::warn("next version equals current — a real run would refuse.");
        }
        if !git_clean(&root) {
            output::warn("working tree is dirty — a real run would refuse.");
        }
        if branch != cfg.require_branch && !args.allow_branch {
            output::warn(format!(
                "not on '{}' — a real run would refuse (use --allow-branch).",
                cfg.require_branch
            ));
        }
        if tag_exists(&root, &tagname) {
            output::warn(format!(
                "tag '{tagname}' already exists — a real run would refuse."
            ));
        }
        output::note("dry run — nothing edited, committed, tagged, or pushed.");
        return Ok(0);
    }

    // Guardrails.
    if next == cur {
        anyhow::bail!("next version equals current ({cur}) — nothing to bump.");
    }
    if !git_clean(&root) {
        anyhow::bail!("working tree is dirty — commit or stash first.");
    }
    if branch != cfg.require_branch && !args.allow_branch {
        anyhow::bail!(
            "not on '{}' (on '{branch}'). Re-run there, or pass --allow-branch.",
            cfg.require_branch
        );
    }
    if tag_exists(&root, &tagname) {
        anyhow::bail!("tag '{tagname}' already exists — refusing to clobber.");
    }

    // Apply the rewrites.
    let mut changed_paths = Vec::new();
    for loc in &cfg.version_locations {
        let path = root.join(&loc.path);
        if !path.is_file() {
            output::warn(format!("missing '{}' — skipped", loc.path));
            continue;
        }
        let content = std::fs::read_to_string(&path)?;
        let (new, n) = rewrite(&content, &loc.anchor, &cur.to_string(), &next.to_string());
        if n > 0 {
            std::fs::write(&path, new)?;
            output::ok(format!("{} → {next}", loc.path));
            changed_paths.push(loc.path.clone());
        }
    }
    if changed_paths.is_empty() {
        anyhow::bail!("no version locations were updated — aborting before commit.");
    }

    // Commit, tag, push.
    git(&root, &format!("git add -- {}", changed_paths.join(" ")))?;
    git(
        &root,
        &format!("git commit -m \"chore: release {tagname}\""),
    )?;
    git(
        &root,
        &format!("git tag -a {tagname} -m \"Release {tagname}\""),
    )?;
    output::info("Pushing commit + tag to origin…");
    git(&root, &format!("git push origin {branch}"))?;
    git(&root, &format!("git push origin {tagname}"))?;

    let _ = state::record(&root, "tag", "passed", &format!("{cur}->{next} {tagname}"));
    output::ok(format!(
        "released {tagname} ({cur} → {next}) and pushed to origin."
    ));
    Ok(0)
}

/// Read the current version from the first location yielding a semver.
fn read_current(root: &Path, cfg: &EffectiveConfig) -> Option<Version> {
    for loc in &cfg.version_locations {
        let content = std::fs::read_to_string(root.join(&loc.path)).ok()?;
        if let Some(v) = extract_version(&content, &loc.anchor) {
            return Some(v);
        }
    }
    None
}

/// Extract the first `"X.Y.Z"` on a line matching `anchor`.
fn extract_version(content: &str, anchor: &str) -> Option<Version> {
    let anchor_re = Regex::new(anchor).ok()?;
    let semver = Regex::new(r#""(\d+\.\d+\.\d+)""#).ok()?;
    for line in content.lines() {
        if anchor_re.is_match(line) {
            if let Some(c) = semver.captures(line) {
                return Version::parse(&c[1]).ok();
            }
        }
    }
    None
}

/// Rewrite `"old"` → `"new"` only on lines matching `anchor`. Returns the new
/// content and the number of lines changed.
fn rewrite(content: &str, anchor: &str, old: &str, new: &str) -> (String, usize) {
    let anchor_re = match Regex::new(anchor) {
        Ok(re) => re,
        Err(_) => return (content.to_string(), 0),
    };
    let needle = format!("\"{old}\"");
    let replacement = format!("\"{new}\"");
    let mut changed = 0;
    let had_trailing_newline = content.ends_with('\n');
    let mut out_lines = Vec::new();
    for line in content.lines() {
        if anchor_re.is_match(line) && line.contains(&needle) {
            out_lines.push(line.replace(&needle, &replacement));
            changed += 1;
        } else {
            out_lines.push(line.to_string());
        }
    }
    let mut out = out_lines.join("\n");
    if had_trailing_newline {
        out.push('\n');
    }
    (out, changed)
}

/// Drafted change lines for dry-run display.
fn plan_changes(
    root: &Path,
    cfg: &EffectiveConfig,
    cur: &Version,
    _next: &Version,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let needle = format!("\"{cur}\"");
    for loc in &cfg.version_locations {
        if let Ok(content) = std::fs::read_to_string(root.join(&loc.path)) {
            if let Ok(re) = Regex::new(&loc.anchor) {
                for line in content.lines() {
                    if re.is_match(line) && line.contains(&needle) {
                        out.push((loc.path.clone(), line.trim().to_string()));
                    }
                }
            }
        }
    }
    out
}

fn tag_prefix(root: &Path, cfg: &EffectiveConfig) -> String {
    match cfg.tag_prefix.as_str() {
        "v" => "v".to_string(),
        "" | "none" => String::new(),
        _ => {
            // auto: bare X.Y.Z tags already? then no prefix; else 'v'.
            let out = process::run_captured("git tag -l", root).ok();
            if let Some(o) = out {
                let bare = o
                    .stdout
                    .lines()
                    .any(|t| Version::parse(t).is_ok() && !t.starts_with('v'));
                let vstyle = o.stdout.lines().any(|t| t.starts_with('v'));
                if bare && !vstyle {
                    return String::new();
                }
            }
            "v".to_string()
        }
    }
}

fn git(root: &Path, cmd: &str) -> anyhow::Result<()> {
    let code = process::run_inherited(cmd, root);
    if code != 0 {
        anyhow::bail!("`{cmd}` failed (exit {code})");
    }
    Ok(())
}

fn git_branch(root: &Path) -> Option<String> {
    let out = process::run_captured("git rev-parse --abbrev-ref HEAD", root).ok()?;
    (out.status == 0).then(|| out.stdout.trim().to_string())
}

fn git_clean(root: &Path) -> bool {
    process::run_captured("git status --porcelain", root)
        .map(|o| o.stdout.trim().is_empty())
        .unwrap_or(false)
}

fn tag_exists(root: &Path, tag: &str) -> bool {
    process::run_captured(&format!("git rev-parse -q --verify refs/tags/{tag}"), root)
        .map(|o| o.status == 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_version_on_anchor_line() {
        let cargo = "[package]\nname = \"x\"\nversion = \"1.4.2\"\n";
        let v = extract_version(cargo, r"^version\s*=").unwrap();
        assert_eq!(v, Version::new(1, 4, 2));
    }

    #[test]
    fn rewrite_only_touches_anchor_lines() {
        // A non-anchor line that also contains the old version must be untouched.
        let src = "version = \"1.0.0\"\nurl = \"https://x/1.0.0\"\n";
        let (out, n) = rewrite(src, r"^version\s*=", "1.0.0", "1.1.0");
        assert_eq!(n, 1);
        assert!(out.contains("version = \"1.1.0\""));
        assert!(out.contains("url = \"https://x/1.0.0\"")); // unchanged
    }

    #[test]
    fn rewrite_preserves_trailing_newline() {
        let (out, _) = rewrite("version = \"1.0.0\"\n", r"^version", "1.0.0", "2.0.0");
        assert!(out.ends_with('\n'));
        assert_eq!(out, "version = \"2.0.0\"\n");
    }

    #[test]
    fn json_version_anchor() {
        let pkg = "{\n  \"version\": \"0.3.0\"\n}\n";
        let v = extract_version(pkg, r#""version"\s*:"#).unwrap();
        assert_eq!(v, Version::new(0, 3, 0));
        let (out, n) = rewrite(pkg, r#""version"\s*:"#, "0.3.0", "0.4.0");
        assert_eq!(n, 1);
        assert!(out.contains("\"version\": \"0.4.0\""));
    }
}
