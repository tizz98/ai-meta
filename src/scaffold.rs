//! Generate the managed artifacts for a project — the `./meta` shim, GitHub
//! workflows, `CLAUDE.md`, `.claude/skills/meta-*`, and `META.md` — from the
//! resolved config + embedded templates. `init` writes these; `upgrade` (P5)
//! regenerates them and diffs. The user-owned `.meta/meta.toml` is generated
//! once here (for init) and thereafter migrated, never clobbered.

use crate::config::EffectiveConfig;
use crate::detect::Detection;
use crate::profile::{CoverageTool, ProfileKind};
use crate::template::{render, Ctx};
use crate::version::{FRAMEWORK_VERSION, SCHEMA_VERSION};

/// Markers delimiting the framework-managed region of `CLAUDE.md`. Everything
/// outside is user prose that `upgrade` must never touch.
pub const MANAGED_START: &str = "<!-- meta:managed:start -->";
pub const MANAGED_END: &str = "<!-- meta:managed:end -->";

/// How an artifact is owned, which decides what `upgrade` does with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    /// Regenerated wholesale by `upgrade`.
    Generated,
    /// Only the managed block is regenerated; surrounding prose is preserved.
    Managed,
    /// Ensure each line is present; never remove user lines.
    AppendMerge,
}

/// A file the framework manages.
#[derive(Debug, Clone)]
pub struct Artifact {
    pub path: String,
    pub content: String,
    pub ownership: Ownership,
    /// Whether the file should be executable (the shim).
    pub executable: bool,
}

const SHIM: &str = include_str!("assets/shim.sh");
const SHIM_CMD: &str = include_str!("assets/shim.cmd");
const SHIM_PS1: &str = include_str!("assets/shim.ps1");
const WF_META_CHECK: &str = include_str!("assets/workflows/meta-check.yml");
const WF_ARCH: &str = include_str!("assets/workflows/arch-review.yml");
const WF_CI: &str = include_str!("assets/workflows/ci.yml");
const CLAUDE_MANAGED: &str = include_str!("assets/claude_managed.md");
const SKILL: &str = include_str!("assets/skill.md");
const META_MD: &str = include_str!("assets/meta.md");

/// Command catalog used to generate per-command skills + docs.
/// (cmd, one-line description, body sentence, usage suffix)
const SKILL_COMMANDS: &[(&str, &str, &str, &str)] = &[
    (
        "status",
        "Project state at a glance.",
        "Show branch, milestone progress, task counts, and the last build/test/check.",
        "",
    ),
    (
        "task",
        "Track work as GitHub issues.",
        "Create/list/show/start/block/done/comment on tasks; state is carried by status:* labels.",
        " <list|show|new|start|block|done|comment>",
    ),
    (
        "milestone",
        "Delivery-milestone progress.",
        "List milestones with completion %, or show a milestone's issues.",
        " <list|show>",
    ),
    (
        "wave",
        "Plan a parallelizable wave.",
        "Plan a wave of ready sub-issues for subagent dispatch (read-only).",
        " <list|show|ready>",
    ),
    (
        "gen",
        "Run configured code generators.",
        "Run the [[codegen]] entries whose trigger files exist.",
        " [--list]",
    ),
    (
        "build",
        "Build the project.",
        "Run the profile/config build command (auto-runs codegen first).",
        "",
    ),
    (
        "test",
        "Run the project's tests.",
        "Run the test command, or the coverage command with --coverage.",
        " [--coverage]",
    ),
    (
        "check",
        "Enforce codified standards.",
        "Run the configured guards; CI runs with --strict.",
        " [--strict] [--json]",
    ),
    (
        "ci",
        "Local merge gate.",
        "Run all gates mirroring CI and post a collapsed PR comment.",
        " [PR]",
    ),
    (
        "arch",
        "Architecture review (advisory).",
        "Flag tech-debt signals and draft refactor tickets.",
        " [--strict] [--json]",
    ),
    (
        "stats",
        "Repo analytics — commits & lines of code.",
        "Show commit counts by author (`commits [--user]`) or lines of code by language (`cloc [--lang]`); every subcommand supports --json.",
        " <commits|cloc> [--json]",
    ),
    (
        "setup",
        "Bootstrap GitHub structure.",
        "Idempotently create labels, milestones, and the project board.",
        " [--dry-run]",
    ),
    (
        "tag",
        "Cut a release.",
        "Bump the version across configured locations, commit, tag, and push.",
        " [major|minor|patch|vX.Y.Z]",
    ),
    (
        "upgrade",
        "Pull framework updates.",
        "Regenerate managed artifacts and migrate meta.toml to a newer ai-meta.",
        " [--dry-run]",
    ),
];

/// All framework-managed artifacts for `cfg` (everything except meta.toml).
pub fn generated_artifacts(cfg: &EffectiveConfig) -> Vec<Artifact> {
    let mut out = Vec::new();

    out.push(Artifact {
        path: ".meta/version".into(),
        content: format!("{FRAMEWORK_VERSION}\n"),
        ownership: Ownership::Generated,
        executable: false,
    });

    out.push(Artifact {
        path: "meta".into(),
        content: SHIM.to_string(),
        ownership: Ownership::Generated,
        executable: true,
    });

    // Windows launchers. `meta.cmd` at the repo root is the dispatcher that
    // PowerShell (`./meta`) and cmd.exe (`.\meta`) both resolve; it relaunches
    // PowerShell with -ExecutionPolicy Bypass into the real launcher, kept in
    // .meta/ so PowerShell never prefers a root `.ps1` (which would be subject
    // to execution policy) over the dispatcher.
    out.push(Artifact {
        path: "meta.cmd".into(),
        content: SHIM_CMD.to_string(),
        ownership: Ownership::Generated,
        executable: false,
    });
    out.push(Artifact {
        path: ".meta/shim.ps1".into(),
        content: SHIM_PS1.to_string(),
        ownership: Ownership::Generated,
        executable: false,
    });

    // Workflows. In self-hosting mode (ai-meta managing itself) CI builds and
    // runs the in-tree binary instead of the pinned `./meta` shim, so the repo
    // validates its own meta.toml with the binary that defines the schema.
    let meta_cmd = if cfg.self_hosting {
        "cargo run --quiet --"
    } else {
        "./meta"
    };
    let wf_ctx = Ctx::new()
        .var("title", &cfg.title)
        .var("meta_cmd", meta_cmd)
        .flag("self_hosting", cfg.self_hosting);
    out.push(Artifact {
        path: ".github/workflows/meta-check.yml".into(),
        content: render(WF_META_CHECK, &wf_ctx),
        ownership: Ownership::Generated,
        executable: false,
    });
    out.push(Artifact {
        path: ".github/workflows/arch-review.yml".into(),
        content: render(WF_ARCH, &wf_ctx),
        ownership: Ownership::Generated,
        executable: false,
    });
    let ci_ctx = Ctx::new()
        .var("toolchain_setup", toolchain_setup(cfg))
        .var("extra_steps", extra_steps(cfg))
        .var("meta_cmd", meta_cmd);
    out.push(Artifact {
        path: ".github/workflows/ci.yml".into(),
        content: render(WF_CI, &ci_ctx),
        ownership: Ownership::Generated,
        executable: false,
    });

    // CLAUDE.md (managed block wrapped in a user-editable file).
    out.push(Artifact {
        path: "CLAUDE.md".into(),
        content: claude_md(cfg),
        ownership: Ownership::Managed,
        executable: false,
    });

    // META.md.
    let meta_ctx = Ctx::new()
        .var("title", &cfg.title)
        .var("profile", cfg.profile_kind.name());
    out.push(Artifact {
        path: "META.md".into(),
        content: render(META_MD, &meta_ctx),
        ownership: Ownership::Generated,
        executable: false,
    });

    // Per-command skills.
    for (cmd, desc, summary, usage) in SKILL_COMMANDS {
        let ctx = Ctx::new()
            .var("cmd", *cmd)
            .var("description", *desc)
            .var("summary", *summary)
            .var("usage_suffix", *usage);
        out.push(Artifact {
            path: format!(".claude/skills/meta-{cmd}/SKILL.md"),
            content: render(SKILL, &ctx),
            ownership: Ownership::Generated,
            executable: false,
        });
    }

    // .gitignore additions (merge, never clobber).
    out.push(Artifact {
        path: ".gitignore".into(),
        content: "/.meta/state/\n/.meta/bin/\n".into(),
        ownership: Ownership::AppendMerge,
        executable: false,
    });

    out
}

/// The rendered managed block (used by `upgrade` to replace in place).
pub fn claude_managed_block(cfg: &EffectiveConfig) -> String {
    let ctx = Ctx::new()
        .flag("has_build", cfg.build.is_some())
        .flag("has_test", cfg.test.is_some())
        .var("build", cfg.build.clone().unwrap_or_default())
        .var("test", cfg.test.clone().unwrap_or_default())
        .var(
            "deps_doc",
            cfg.deps_doc
                .clone()
                .unwrap_or_else(|| "docs/dependencies.md".into()),
        );
    // Normalize surrounding newlines so the block round-trips identically
    // through `replace_managed` (which trims) on every `upgrade`.
    render(CLAUDE_MANAGED, &ctx).trim_matches('\n').to_string()
}

fn claude_md(cfg: &EffectiveConfig) -> String {
    let intro = cfg
        .description
        .clone()
        .unwrap_or_else(|| format!("{} — project instructions.", cfg.title));
    format!(
        "# CLAUDE.md — {}\n\n{}\n\n{}\n{}\n{}\n",
        cfg.title,
        intro,
        MANAGED_START,
        claude_managed_block(cfg),
        MANAGED_END
    )
}

fn toolchain_setup(cfg: &EffectiveConfig) -> String {
    match cfg.profile_kind {
        ProfileKind::Rust => {
            let mut s = String::from(
                "      - uses: dtolnay/rust-toolchain@stable\n        with:\n          components: clippy, rustfmt\n",
            );
            if cfg.coverage_min > 0 && matches!(cfg.coverage_tool, CoverageTool::CargoLlvmCov) {
                s.push_str("      - name: Install cargo-llvm-cov\n        uses: taiki-e/install-action@cargo-llvm-cov\n");
            }
            s
        }
        ProfileKind::TypeScript => {
            "      - uses: actions/setup-node@v4\n        with:\n          node-version: 20\n          cache: npm\n      - run: npm ci\n".into()
        }
        ProfileKind::Python => {
            "      - uses: actions/setup-python@v5\n        with:\n          python-version: \"3.12\"\n      - name: Install deps\n        run: pip install -e . || pip install -r requirements.txt || true\n".into()
        }
        ProfileKind::Generic => String::new(),
    }
}

fn extra_steps(cfg: &EffectiveConfig) -> String {
    let mut s = String::new();
    for step in &cfg.ci_extra_steps {
        if step.workflow != "ci" {
            continue;
        }
        s.push_str(&format!(
            "      - name: {}\n        run: {}\n",
            step.name, step.run
        ));
    }
    s
}

/// Render the initial `.meta/meta.toml` for `init`, seeded from detection.
pub fn render_meta_toml(
    kind: ProfileKind,
    title: &str,
    description: Option<&str>,
    detect: Option<&Detection>,
) -> String {
    let mut s = String::new();
    s.push_str("# .meta/meta.toml — managed by ai-meta (https://github.com/tizz98/ai-meta).\n");
    s.push_str("# Profile defaults are baked into the binary; this file only records identity\n");
    s.push_str("# and overrides. Run `meta upgrade` to pull framework updates.\n\n");
    s.push_str("[meta]\n");
    s.push_str(&format!("schema_version = {SCHEMA_VERSION}\n"));
    s.push_str(&format!("framework_version = \"{FRAMEWORK_VERSION}\"\n"));
    s.push_str(&format!("profile = \"{}\"\n\n", kind.name()));
    s.push_str("[project]\n");
    s.push_str(&format!("title = {}\n", toml_str(title)));
    if let Some(d) = description {
        s.push_str(&format!("description = {}\n", toml_str(d)));
    }
    s.push('\n');

    // Inferred command overrides (only those positively detected).
    if let Some(d) = detect {
        let c = &d.commands;
        let any = c.build.is_some()
            || c.test.is_some()
            || c.fmt.is_some()
            || c.lint.is_some()
            || c.typecheck.is_some()
            || c.coverage.is_some();
        if any {
            s.push_str("# Commands inferred from the repo (override the profile defaults).\n");
            s.push_str("[commands]\n");
            for (k, v) in [
                ("build", &c.build),
                ("test", &c.test),
                ("fmt", &c.fmt),
                ("lint", &c.lint),
                ("typecheck", &c.typecheck),
                ("coverage", &c.coverage),
            ] {
                if let Some(val) = v {
                    s.push_str(&format!("{k} = {}\n", toml_str(val)));
                }
            }
            s.push('\n');
        }
        if !d.domains.is_empty() {
            s.push_str("[github]\n");
            let items: Vec<String> = d.domains.iter().map(|x| toml_str(x)).collect();
            s.push_str(&format!("domains = [{}]\n\n", items.join(", ")));
        }
    }

    s.push_str("# Add milestones, label colors, dep allowlist, and rule tuning as needed.\n");
    s.push_str("# See `meta` docs; everything omitted inherits the profile default.\n");
    s
}

fn toml_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Compute the on-disk content for an artifact given what's already there. This
/// is what `init`/`upgrade` write. `Generated` overwrites; `Managed` replaces
/// only the marker block (preserving user prose); `AppendMerge` ensures lines.
pub fn resolve_content(artifact: &Artifact, existing: Option<&str>) -> String {
    match artifact.ownership {
        Ownership::Generated => artifact.content.clone(),
        Ownership::Managed => match existing {
            Some(prev) if prev.contains(MANAGED_START) && prev.contains(MANAGED_END) => {
                replace_managed(prev, &artifact.content)
            }
            // No existing managed block: write the freshly generated full file.
            _ => artifact.content.clone(),
        },
        Ownership::AppendMerge => merge_lines(existing.unwrap_or(""), &artifact.content),
    }
}

/// Replace the managed region of `existing` with the managed region of
/// `generated` (both contain the markers). User prose outside is preserved.
fn replace_managed(existing: &str, generated: &str) -> String {
    let block = extract_managed(generated).unwrap_or_default();
    let start = existing.find(MANAGED_START).unwrap_or(0);
    let end = existing
        .find(MANAGED_END)
        .map(|e| e + MANAGED_END.len())
        .unwrap_or(existing.len());
    let mut out = String::new();
    out.push_str(&existing[..start]);
    out.push_str(MANAGED_START);
    out.push('\n');
    out.push_str(&block);
    out.push('\n');
    out.push_str(MANAGED_END);
    out.push_str(&existing[end..]);
    out
}

/// The text strictly between the managed markers (trimmed of the marker lines).
fn extract_managed(s: &str) -> Option<String> {
    let start = s.find(MANAGED_START)? + MANAGED_START.len();
    let end = s.find(MANAGED_END)?;
    Some(s[start..end].trim_matches('\n').to_string())
}

/// Write `content` to `path`, creating parents, optionally setting the
/// executable bit (used for the shim). Shared by `init` and `upgrade`.
pub fn write_file(path: &std::path::Path, content: &str, executable: bool) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    if executable {
        set_executable(path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn set_executable(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

/// Ensure every non-empty line of `additions` is present in `existing`.
fn merge_lines(existing: &str, additions: &str) -> String {
    let mut out = existing.to_string();
    let have: std::collections::HashSet<&str> = existing.lines().map(|l| l.trim()).collect();
    let mut appended = false;
    for line in additions.lines() {
        let t = line.trim();
        if t.is_empty() || have.contains(t) {
            continue;
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if !appended && !existing.is_empty() {
            out.push_str("\n# ai-meta\n");
            appended = true;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use std::path::PathBuf;

    fn cfg(profile: &str) -> EffectiveConfig {
        let toml = format!("[meta]\nprofile = \"{profile}\"\n[project]\ntitle = \"demo\"\n");
        config::load_from_str(&PathBuf::from("/tmp/demo"), &toml).unwrap()
    }

    #[test]
    fn generates_expected_artifacts_for_rust() {
        let arts = generated_artifacts(&cfg("rust"));
        let paths: Vec<_> = arts.iter().map(|a| a.path.as_str()).collect();
        assert!(paths.contains(&"meta"));
        assert!(paths.contains(&".meta/version"));
        assert!(paths.contains(&".github/workflows/ci.yml"));
        assert!(paths.contains(&"CLAUDE.md"));
        assert!(paths.contains(&"META.md"));
        assert!(paths
            .iter()
            .any(|p| p.starts_with(".claude/skills/meta-check/")));
        // shim is executable
        assert!(arts.iter().find(|a| a.path == "meta").unwrap().executable);
    }

    #[test]
    fn generates_stats_skill_and_docs() {
        let arts = generated_artifacts(&cfg("rust"));
        let skill = arts
            .iter()
            .find(|a| a.path == ".claude/skills/meta-stats/SKILL.md")
            .expect("meta-stats skill artifact");
        assert!(skill.content.contains("commits"));
        assert!(skill.content.contains("cloc"));
        assert!(skill.content.contains("--json"));

        let meta_md = arts.iter().find(|a| a.path == "META.md").unwrap();
        assert!(meta_md.content.contains("`./meta stats`"));
    }

    #[test]
    fn generates_windows_launchers() {
        let arts = generated_artifacts(&cfg("rust"));
        let cmd = arts
            .iter()
            .find(|a| a.path == "meta.cmd")
            .expect("meta.cmd artifact");
        let ps1 = arts
            .iter()
            .find(|a| a.path == ".meta/shim.ps1")
            .expect(".meta/shim.ps1 artifact");
        // Both are wholesale-generated, non-executable data files.
        assert_eq!(cmd.ownership, Ownership::Generated);
        assert_eq!(ps1.ownership, Ownership::Generated);
        assert!(!cmd.executable);
        assert!(!ps1.executable);
        // The dispatcher relaunches PowerShell with a policy bypass into the
        // hidden shim; the shim targets the published Windows triple.
        assert!(cmd.content.contains(".meta\\shim.ps1"));
        assert!(cmd.content.contains("-ExecutionPolicy Bypass"));
        assert!(ps1.content.contains("x86_64-pc-windows-msvc"));
        // Crucially, NO root-level meta.ps1 (that would let PowerShell bypass the
        // dispatcher and hit execution policy).
        assert!(arts.iter().all(|a| a.path != "meta.ps1"));
    }

    #[test]
    fn ci_workflow_has_rust_toolchain() {
        let arts = generated_artifacts(&cfg("rust"));
        let ci = arts.iter().find(|a| a.path.ends_with("ci.yml")).unwrap();
        assert!(ci.content.contains("dtolnay/rust-toolchain"));
        assert!(ci.content.contains("./meta ci"));
    }

    #[test]
    fn ci_workflow_has_node_for_ts() {
        let arts = generated_artifacts(&cfg("typescript"));
        let ci = arts.iter().find(|a| a.path.ends_with("ci.yml")).unwrap();
        assert!(ci.content.contains("setup-node"));
    }

    fn self_hosting_cfg() -> EffectiveConfig {
        let toml = "[meta]\nprofile = \"rust\"\nself_hosting = true\n[project]\ntitle = \"demo\"\n";
        config::load_from_str(&PathBuf::from("/tmp/demo"), toml).unwrap()
    }

    #[test]
    fn self_hosting_workflows_build_in_tree_binary() {
        let arts = generated_artifacts(&self_hosting_cfg());
        let get = |suffix: &str| {
            arts.iter()
                .find(|a| a.path.ends_with(suffix))
                .unwrap()
                .content
                .clone()
        };
        let check = get("meta-check.yml");
        assert!(check.contains("cargo run --quiet -- check --strict"));
        assert!(check.contains("dtolnay/rust-toolchain"));
        assert!(!check.contains("./meta check"));

        let arch = get("arch-review.yml");
        assert!(arch.contains("cargo run --quiet -- arch || true"));
        assert!(arch.contains("dtolnay/rust-toolchain"));

        let ci = get("ci.yml");
        assert!(ci.contains("cargo run --quiet -- ci"));
        assert!(!ci.contains("./meta ci"));
        assert!(ci.contains("dtolnay/rust-toolchain"));
    }

    #[test]
    fn non_self_hosting_workflows_use_the_shim() {
        let arts = generated_artifacts(&cfg("rust"));
        let check = arts
            .iter()
            .find(|a| a.path.ends_with("meta-check.yml"))
            .unwrap();
        assert!(check.content.contains("./meta check --strict"));
        assert!(!check.content.contains("cargo run"));
        assert!(!check.content.contains("dtolnay/rust-toolchain"));

        let arch = arts
            .iter()
            .find(|a| a.path.ends_with("arch-review.yml"))
            .unwrap();
        assert!(arch.content.contains("./meta arch || true"));
        assert!(!arch.content.contains("cargo run"));
        assert!(!arch.content.contains("dtolnay/rust-toolchain"));
    }

    #[test]
    fn claude_md_has_managed_markers() {
        let arts = generated_artifacts(&cfg("rust"));
        let cm = arts.iter().find(|a| a.path == "CLAUDE.md").unwrap();
        assert!(cm.content.contains(MANAGED_START));
        assert!(cm.content.contains(MANAGED_END));
        assert_eq!(cm.ownership, Ownership::Managed);
    }

    #[test]
    fn meta_toml_includes_inferred_commands() {
        let mut detect = Detection::default();
        detect.commands.build = Some("cargo build".into());
        detect.domains = vec!["core".into(), "server".into()];
        let toml = render_meta_toml(ProfileKind::Rust, "demo", Some("a demo"), Some(&detect));
        assert!(toml.contains("profile = \"rust\""));
        assert!(toml.contains("build = \"cargo build\""));
        assert!(toml.contains("domains = [\"core\", \"server\"]"));
        // and it parses back cleanly
        config::load_from_str(&PathBuf::from("/tmp/x"), &toml).unwrap();
    }
}
