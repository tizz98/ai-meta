//! Profile + command/structure **inference** for an existing repo, so
//! `meta init` is near-zero-config. We look at manifest markers to pick the
//! profile, then read those manifests to infer command overrides and domains.
//! Anything not detected falls back to the profile default (and `init` leaves it
//! commented in meta.toml so the user can see and change it).

use crate::profile::ProfileKind;
use std::path::Path;

/// The outcome of inspecting a repo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detection {
    pub kind: Option<ProfileKind>,
    /// Marker files that drove the decision (e.g. "Cargo.toml").
    pub markers: Vec<String>,
    /// Inferred command overrides (only those that differ from / refine the
    /// profile default, or that were positively detected).
    pub commands: InferredCommands,
    /// Inferred domain labels (top-level source areas / workspace members).
    pub domains: Vec<String>,
    /// Human-readable notes (ambiguities, fallbacks).
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InferredCommands {
    pub build: Option<String>,
    pub test: Option<String>,
    pub fmt: Option<String>,
    pub lint: Option<String>,
    pub typecheck: Option<String>,
    pub coverage: Option<String>,
}

/// Inspect `root`, returning the detected profile + inferred config.
pub fn detect(root: &Path) -> Detection {
    let mut det = Detection::default();

    let has = |f: &str| root.join(f).exists();

    // Profile signal, in priority order. Record any other markers seen.
    if has("Cargo.toml") {
        det.kind = Some(ProfileKind::Rust);
        det.markers.push("Cargo.toml".into());
        infer_rust(root, &mut det);
    } else if has("package.json") {
        det.kind = Some(ProfileKind::TypeScript);
        det.markers.push("package.json".into());
        infer_typescript(root, &mut det);
    } else if has("pyproject.toml")
        || has("setup.py")
        || has("setup.cfg")
        || has("requirements.txt")
    {
        det.kind = Some(ProfileKind::Python);
        for m in [
            "pyproject.toml",
            "setup.py",
            "setup.cfg",
            "requirements.txt",
        ] {
            if has(m) {
                det.markers.push(m.into());
            }
        }
        infer_python(root, &mut det);
    } else {
        det.kind = Some(ProfileKind::Generic);
        det.notes
            .push("no language markers found; falling back to the generic profile".into());
    }

    // Note secondary markers (e.g. a Rust workspace that also ships JS SDKs).
    for (other, kind) in [
        ("package.json", ProfileKind::TypeScript),
        ("pyproject.toml", ProfileKind::Python),
    ] {
        if det.kind != Some(kind) && has(other) {
            det.notes.push(format!(
                "also saw {other}; chose {} by priority",
                det.kind.map(|k| k.name()).unwrap_or("generic")
            ));
        }
    }

    det
}

fn infer_rust(root: &Path, det: &mut Detection) {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    let is_workspace = manifest.contains("[workspace]");
    if !is_workspace {
        // Single crate: drop the --workspace flag.
        det.commands.build = Some("cargo build".into());
        det.commands.test = Some("cargo test".into());
        det.notes.push("single crate (no [workspace])".into());
    }
    // Workspace members as domains.
    if is_workspace {
        det.domains = workspace_members(&manifest);
    }
    if det.domains.is_empty() {
        det.domains = child_dirs(root, &["src"]);
    }
}

fn infer_typescript(root: &Path, det: &mut Detection) {
    let pkg = std::fs::read_to_string(root.join("package.json")).unwrap_or_default();
    let scripts = json_scripts(&pkg);

    let has_script = |name: &str| scripts.iter().any(|s| s == name);
    if has_script("build") {
        det.commands.build = Some("npm run build".into());
    }
    if has_script("test") {
        det.commands.test = Some("npm test".into());
    }
    if has_script("lint") {
        det.commands.lint = Some("npm run lint".into());
    }
    if has_script("typecheck") {
        det.commands.typecheck = Some("npm run typecheck".into());
    }
    if has_script("test:coverage") {
        det.commands.coverage = Some("npm run test:coverage".into());
    } else if has_script("coverage") {
        det.commands.coverage = Some("npm run coverage".into());
    }
    if !scripts.is_empty() {
        det.notes.push(format!(
            "inferred from package.json scripts: {}",
            scripts.join(", ")
        ));
    }
    det.domains = child_dirs(&root.join("src"), &[]);
    if det.domains.is_empty() {
        det.domains = child_dirs(root, &["src"]);
    }
}

fn infer_python(root: &Path, det: &mut Detection) {
    let pyproject = std::fs::read_to_string(root.join("pyproject.toml")).unwrap_or_default();
    let blob = format!(
        "{pyproject}\n{}",
        std::fs::read_to_string(root.join("requirements.txt")).unwrap_or_default()
    );
    let mentions = |needle: &str| blob.contains(needle);

    if mentions("ruff") {
        det.commands.lint = Some("ruff check".into());
        det.commands.fmt = Some("ruff format --check".into());
    } else if mentions("flake8") {
        det.commands.lint = Some("flake8".into());
    }
    if mentions("pyright") {
        det.commands.typecheck = Some("pyright".into());
    } else if mentions("mypy") {
        det.commands.typecheck = Some("mypy .".into());
    }
    if mentions("pytest") || root.join("tests").is_dir() {
        det.commands.test = Some("pytest".into());
        det.commands.coverage = Some("pytest --cov".into());
    }
    det.domains = child_dirs(&root.join("src"), &[]);
    if det.domains.is_empty() {
        det.domains = python_packages(root);
    }
}

/// Extract `[workspace] members = [...]` leaf directory names from a Cargo
/// manifest. Best-effort line scan (avoids a full TOML parse for globs).
fn workspace_members(manifest: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(start) = manifest.find("members") {
        if let Some(open) = manifest[start..].find('[') {
            let from = start + open + 1;
            if let Some(close) = manifest[from..].find(']') {
                let body = &manifest[from..from + close];
                for raw in body.split(',') {
                    let m = raw.trim().trim_matches(['"', '\'']).trim();
                    if m.is_empty() || m.contains('*') {
                        continue;
                    }
                    let leaf = m.rsplit('/').next().unwrap_or(m);
                    out.push(leaf.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

/// Parse the keys of the `scripts` object from package.json.
fn json_scripts(pkg: &str) -> Vec<String> {
    let v: serde_json::Value = match serde_json::from_str(pkg) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    v.get("scripts")
        .and_then(|s| s.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

/// Immediate child directory names of `dir`, excluding any in `skip` and dotted
/// dirs. Returns sorted names.
fn child_dirs(dir: &Path, skip: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || skip.contains(&name.as_str()) {
                continue;
            }
            out.push(name);
        }
    }
    out.sort();
    out
}

/// Directories at root that look like importable Python packages (`__init__.py`).
fn python_packages(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            if e.path().is_dir() && e.path().join("__init__.py").is_file() {
                out.push(e.file_name().to_string_lossy().to_string());
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detects_rust_workspace_and_members() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"core\", \"protocol/codec\", \"server\"]\n",
        )
        .unwrap();
        let d = detect(tmp.path());
        assert_eq!(d.kind, Some(ProfileKind::Rust));
        assert!(d.commands.build.is_none()); // workspace -> keep default
        assert_eq!(d.domains, vec!["codec", "core", "server"]);
    }

    #[test]
    fn detects_single_rust_crate() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        let d = detect(tmp.path());
        assert_eq!(d.kind, Some(ProfileKind::Rust));
        assert_eq!(d.commands.build.as_deref(), Some("cargo build"));
        assert_eq!(d.commands.test.as_deref(), Some("cargo test"));
    }

    #[test]
    fn detects_typescript_and_infers_scripts() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("package.json"),
            r#"{ "scripts": { "build": "tsc", "test": "vitest run", "typecheck": "tsc --noEmit", "test:coverage": "vitest run --coverage" } }"#,
        )
        .unwrap();
        fs::create_dir_all(tmp.path().join("src").join("bot")).unwrap();
        fs::create_dir_all(tmp.path().join("src").join("engine")).unwrap();
        let d = detect(tmp.path());
        assert_eq!(d.kind, Some(ProfileKind::TypeScript));
        assert_eq!(d.commands.typecheck.as_deref(), Some("npm run typecheck"));
        assert_eq!(
            d.commands.coverage.as_deref(),
            Some("npm run test:coverage")
        );
        assert_eq!(d.domains, vec!["bot", "engine"]);
    }

    #[test]
    fn detects_python_tools() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("pyproject.toml"),
            "[tool.ruff]\n[tool.mypy]\n[tool.pytest.ini_options]\n",
        )
        .unwrap();
        let d = detect(tmp.path());
        assert_eq!(d.kind, Some(ProfileKind::Python));
        assert_eq!(d.commands.lint.as_deref(), Some("ruff check"));
        assert_eq!(d.commands.typecheck.as_deref(), Some("mypy ."));
        assert_eq!(d.commands.test.as_deref(), Some("pytest"));
    }

    #[test]
    fn falls_back_to_generic() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("README.md"), "# hi").unwrap();
        let d = detect(tmp.path());
        assert_eq!(d.kind, Some(ProfileKind::Generic));
        assert!(!d.notes.is_empty());
    }

    #[test]
    fn notes_secondary_markers() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        let d = detect(tmp.path());
        assert_eq!(d.kind, Some(ProfileKind::Rust));
        assert!(d.notes.iter().any(|n| n.contains("package.json")));
    }
}
