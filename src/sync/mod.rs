//! Compute and apply the set of changes `meta upgrade` makes: regenerate the
//! managed artifacts wholesale (Generated), update only the managed block of
//! Managed files, merge into AppendMerge files, and migrate the user-owned
//! meta.toml — honoring `[sync] ignore` and supporting a dry-run diff.

pub mod compat;
pub mod diff;

use crate::config::EffectiveConfig;
use crate::scaffold::{self, Artifact};
use crate::version::FRAMEWORK_VERSION;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    New,
    Modified,
    Unchanged,
}

/// A planned change to one file.
#[derive(Debug, Clone)]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
    pub old: String,
    pub new: String,
    pub executable: bool,
    pub ignored: bool,
}

/// The full upgrade plan.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub changes: Vec<Change>,
}

impl Plan {
    pub fn modified(&self) -> impl Iterator<Item = &Change> {
        self.changes
            .iter()
            .filter(|c| c.kind != ChangeKind::Unchanged && !c.ignored)
    }
}

/// Build the upgrade plan. `meta_old`/`meta_new` are the user-owned meta.toml
/// before/after migration + repin. `target` is the framework version to pin.
pub fn plan(
    root: &Path,
    cfg: &EffectiveConfig,
    meta_old: &str,
    meta_new: &str,
    target: &str,
    ignore: &[String],
) -> Plan {
    let mut changes = Vec::new();

    // The user-owned config (migrated + repinned, never wholesale-regenerated).
    changes.push(classify(
        root,
        ".meta/meta.toml",
        meta_old.to_string(),
        meta_new.to_string(),
        false,
        ignore,
    ));

    for art in scaffold::generated_artifacts(cfg) {
        let new = artifact_content(root, &art, target);
        let old = std::fs::read_to_string(root.join(&art.path)).unwrap_or_default();
        changes.push(classify(root, &art.path, old, new, art.executable, ignore));
    }

    Plan { changes }
}

/// Resolve an artifact's target content, overriding the pinned version when an
/// explicit `--to` target differs from this binary's version.
fn artifact_content(root: &Path, art: &Artifact, target: &str) -> String {
    if art.path == ".meta/version" && target != FRAMEWORK_VERSION {
        return format!("{target}\n");
    }
    let existing = std::fs::read_to_string(root.join(&art.path)).ok();
    scaffold::resolve_content(art, existing.as_deref())
}

fn classify(
    root: &Path,
    path: &str,
    old: String,
    new: String,
    executable: bool,
    ignore: &[String],
) -> Change {
    let exists = root.join(path).exists();
    let kind = if !exists {
        ChangeKind::New
    } else if old != new {
        ChangeKind::Modified
    } else {
        ChangeKind::Unchanged
    };
    Change {
        path: path.to_string(),
        kind,
        old,
        new,
        executable,
        ignored: ignore.iter().any(|i| i == path),
    }
}

/// Result counts from applying a plan.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApplySummary {
    pub updated: usize,
    pub unchanged: usize,
    pub ignored: usize,
}

/// Write all modified, non-ignored changes to disk.
pub fn apply(root: &Path, plan: &Plan) -> std::io::Result<ApplySummary> {
    let mut s = ApplySummary::default();
    for c in &plan.changes {
        if c.ignored {
            if c.kind != ChangeKind::Unchanged {
                s.ignored += 1;
            }
            continue;
        }
        match c.kind {
            ChangeKind::Unchanged => s.unchanged += 1,
            _ => {
                scaffold::write_file(&root.join(&c.path), &c.new, c.executable)?;
                s.updated += 1;
            }
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use std::fs;
    use tempfile::tempdir;

    fn cfg_for(root: &Path) -> (EffectiveConfig, String) {
        let toml = "[meta]\nschema_version = 1\nframework_version = \"0.1.0\"\nprofile = \"rust\"\n[project]\ntitle = \"demo\"\n";
        (config::load_from_str(root, toml).unwrap(), toml.to_string())
    }

    #[test]
    fn fresh_repo_all_new_then_idempotent() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let (cfg, meta) = cfg_for(root);

        let p = plan(root, &cfg, &meta, &meta, FRAMEWORK_VERSION, &[]);
        assert!(p.changes.iter().all(|c| c.kind == ChangeKind::New));
        apply(root, &p).unwrap();

        // Second plan after apply: everything unchanged (idempotent).
        let p2 = plan(root, &cfg, &meta, &meta, FRAMEWORK_VERSION, &[]);
        assert_eq!(p2.modified().count(), 0);
    }

    #[test]
    fn ignored_paths_are_not_written() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let (cfg, meta) = cfg_for(root);
        let ignore = vec![".github/workflows/ci.yml".to_string()];
        let p = plan(root, &cfg, &meta, &meta, FRAMEWORK_VERSION, &ignore);
        apply(root, &p).unwrap();
        assert!(!root.join(".github/workflows/ci.yml").exists());
        // but other files were written
        assert!(root.join("meta").exists());
    }

    #[test]
    fn preserves_user_prose_in_claude_md() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let (cfg, meta) = cfg_for(root);

        // First write everything.
        apply(root, &plan(root, &cfg, &meta, &meta, FRAMEWORK_VERSION, &[])).unwrap();

        // User edits prose outside the managed markers.
        let claude_path = root.join("CLAUDE.md");
        let mut content = fs::read_to_string(&claude_path).unwrap();
        content.push_str("\n## My notes\nhand-written.\n");
        fs::write(&claude_path, &content).unwrap();

        // Re-plan/apply: managed block regenerates, user prose survives.
        apply(root, &plan(root, &cfg, &meta, &meta, FRAMEWORK_VERSION, &[])).unwrap();
        let after = fs::read_to_string(&claude_path).unwrap();
        assert!(after.contains("## My notes"));
        assert!(after.contains("hand-written."));
        assert!(after.contains(scaffold::MANAGED_START));
    }
}
