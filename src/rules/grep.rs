//! The source scanner under the rule engine: enumerate a repo's source files
//! (honoring the same vendored-dir excludes the bash `_meta_grep` used) and
//! match regex patterns over them. Replaces `_meta_grep`/`_scan`/`_*_files`.

use regex::Regex;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Directories never scanned (build output, vcs, vendored deps, our own config).
const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".meta",
    ".claude",
    "target",
    "node_modules",
    "dist",
    "build",
    "coverage",
    ".venv",
    "venv",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
];

/// A single match: `file:line:text` once rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub file: String,
    pub line: usize,
    pub text: String,
}

impl Hit {
    /// Render as the bash `file:line:match` form (text trimmed of trailing ws).
    pub fn render(&self) -> String {
        format!("{}:{}:{}", self.file, self.line, self.text.trim_end())
    }
}

/// Whether `path` (relative to a scan root) is a test/bench/declaration file
/// that guards exclude. `exts` are the language source extensions.
pub fn is_test_path(rel: &str, exts: &[String]) -> bool {
    if rel.contains("/tests/") || rel.starts_with("tests/") || rel.contains("/benches/") {
        return true;
    }
    if rel.ends_with(".d.ts") {
        return true;
    }
    // `_test.<ext>` (rust/go style) and `.test.`/`.spec.` (js/ts) and python
    // `test_*.py` / `*_test.py`.
    for e in exts {
        if rel.ends_with(&format!("_test.{e}")) {
            return true;
        }
    }
    let base = rel.rsplit('/').next().unwrap_or(rel);
    base.contains(".test.")
        || base.contains(".spec.")
        || base.starts_with("test_")
        || base == "conftest.py"
}

/// Collect source files under `root` with one of `exts`, sorted for determinism.
/// When `roots` is non-empty, only those subdirectories of `root` are scanned.
pub fn collect_files(root: &Path, exts: &[String], roots: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let bases: Vec<PathBuf> = if roots.is_empty() {
        vec![root.to_path_buf()]
    } else {
        roots.iter().map(|r| root.join(r)).collect()
    };
    for base in bases {
        if !base.exists() {
            continue;
        }
        for entry in WalkDir::new(&base)
            .into_iter()
            .filter_entry(|e| !is_excluded_dir(e.path()))
            .flatten()
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let p = entry.path();
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if exts.iter().any(|e| e == ext) {
                    out.push(p.to_path_buf());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn is_excluded_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| EXCLUDED_DIRS.contains(&n))
        .unwrap_or(false)
}

/// Path relative to `root`, using forward slashes, for display + path checks.
pub fn rel_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Read a file to a string, or empty on error (a binary/unreadable file simply
/// contributes no matches — guards must never crash).
pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// Match `re` line-by-line over `text`, returning (1-based line, line text).
pub fn match_lines(text: &str, re: &Regex) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| re.is_match(l))
        .map(|(i, l)| (i + 1, l.to_string()))
        .collect()
}

/// Count lines matching `re` in `text`.
pub fn count_matches(text: &str, re: &Regex) -> usize {
    text.lines().filter(|l| re.is_match(l)).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn collects_only_matching_exts_and_skips_vendored() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::create_dir_all(tmp.path().join("target")).unwrap();
        fs::write(tmp.path().join("src/a.rs"), "fn a(){}").unwrap();
        fs::write(tmp.path().join("src/b.txt"), "nope").unwrap();
        fs::write(tmp.path().join("target/gen.rs"), "fn g(){}").unwrap();
        let files = collect_files(tmp.path(), &["rs".into()], &[]);
        let rels: Vec<_> = files.iter().map(|p| rel_display(tmp.path(), p)).collect();
        assert_eq!(rels, vec!["src/a.rs"]);
    }

    #[test]
    fn test_path_detection() {
        let exts = vec!["rs".into(), "ts".into(), "py".into()];
        assert!(is_test_path("crate/tests/it.rs", &exts));
        assert!(is_test_path("src/foo_test.rs", &exts));
        assert!(is_test_path("src/foo.test.ts", &exts));
        assert!(is_test_path("src/foo.spec.ts", &exts));
        assert!(is_test_path("pkg/test_foo.py", &exts));
        assert!(is_test_path("types/x.d.ts", &exts));
        assert!(!is_test_path("src/foo.rs", &exts));
        assert!(!is_test_path("src/testing.rs", &exts));
    }

    #[test]
    fn match_and_count() {
        let re = Regex::new(r"unwrap\(\)").unwrap();
        let text = "let x = a.unwrap();\nlet y = b;\nc.unwrap()\n";
        assert_eq!(count_matches(text, &re), 2);
        let m = match_lines(text, &re);
        assert_eq!(m[0].0, 1);
        assert_eq!(m[1].0, 3);
    }
}
