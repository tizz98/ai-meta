//! The run-result cache under `.meta/state/<key>`, mirroring the bash
//! `meta_state_record/read/describe`. One file per key, format
//! `STATUS|EPOCH|DETAIL`, gitignored. Used by `status` and the gate commands.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A recorded run outcome for one key (build, test, check, ci, arch, tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub status: String,
    pub epoch: u64,
    pub detail: String,
}

fn state_dir(root: &Path) -> PathBuf {
    root.join(".meta").join("state")
}

fn key_path(root: &Path, key: &str) -> PathBuf {
    state_dir(root).join(key)
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Record an outcome. Creates `.meta/state/` if needed. Best-effort: a write
/// failure is returned but callers generally ignore it (the cache is advisory).
pub fn record(root: &Path, key: &str, status: &str, detail: &str) -> std::io::Result<()> {
    let dir = state_dir(root);
    fs::create_dir_all(&dir)?;
    let line = format!("{}|{}|{}", status, now_epoch(), detail);
    fs::write(key_path(root, key), line)
}

/// Read a recorded outcome, or `None` if absent/unparseable.
pub fn read(root: &Path, key: &str) -> Option<Record> {
    let raw = fs::read_to_string(key_path(root, key)).ok()?;
    parse(raw.trim())
}

fn parse(raw: &str) -> Option<Record> {
    let mut parts = raw.splitn(3, '|');
    let status = parts.next()?.to_string();
    let epoch = parts.next()?.parse::<u64>().ok()?;
    let detail = parts.next().unwrap_or("").to_string();
    Some(Record {
        status,
        epoch,
        detail,
    })
}

/// Human-readable "passed (3h ago)" describing the key, or "never run".
pub fn describe(root: &Path, key: &str) -> String {
    match read(root, key) {
        Some(r) => format!("{} ({})", r.status, ago(now_epoch().saturating_sub(r.epoch))),
        None => "never run".to_string(),
    }
}

/// Format a duration-in-seconds as a coarse "Ns/Nm/Nh/Nd ago".
fn ago(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86_400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_then_read_roundtrips() {
        let tmp = tempfile::tempdir().unwrap();
        record(tmp.path(), "build", "passed", "cargo build --workspace").unwrap();
        let r = read(tmp.path(), "build").unwrap();
        assert_eq!(r.status, "passed");
        assert_eq!(r.detail, "cargo build --workspace");
    }

    #[test]
    fn detail_may_contain_pipes() {
        let tmp = tempfile::tempdir().unwrap();
        record(tmp.path(), "ci", "failed", "a|b|c").unwrap();
        let r = read(tmp.path(), "ci").unwrap();
        assert_eq!(r.detail, "a|b|c");
    }

    #[test]
    fn missing_key_is_none_and_never_run() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read(tmp.path(), "nope").is_none());
        assert_eq!(describe(tmp.path(), "nope"), "never run");
    }

    #[test]
    fn ago_buckets() {
        assert_eq!(ago(5), "5s ago");
        assert_eq!(ago(120), "2m ago");
        assert_eq!(ago(7200), "2h ago");
        assert_eq!(ago(172_800), "2d ago");
    }
}
