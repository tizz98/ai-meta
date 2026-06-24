//! Schema migrations for the user-owned `.meta/meta.toml`. Migrations are
//! applied with `toml_edit` so comments, ordering, and user values survive —
//! they only add/rename/transform structurally, never clobber. The registry is
//! an ordered list of `from_version -> migration`; adding a breaking config
//! change means bumping [`crate::version::SCHEMA_VERSION`] and appending one fn.

use crate::error::{Error, Result};
use crate::version::SCHEMA_VERSION;
use toml_edit::{value, DocumentMut, Item};

/// A single forward migration from schema version `from` to `from + 1`.
struct Migration {
    from: u32,
    apply: fn(&mut DocumentMut),
}

/// The ordered migration registry. Empty today (schema v1 is the first), but the
/// machinery is in place: append `Migration { from: 1, apply: v1_to_v2 }` when
/// the schema changes.
const MIGRATIONS: &[Migration] = &[];

/// Migrate `text` up to the current [`SCHEMA_VERSION`], preserving formatting.
/// Returns the rewritten text and the list of applied step descriptions.
pub fn migrate(text: &str) -> Result<(String, Vec<String>)> {
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| Error::Config(format!("meta.toml: {e}")))?;

    let mut current = read_schema_version(&doc);
    if current > SCHEMA_VERSION {
        return Err(Error::SchemaTooNew {
            repo: current,
            binary: SCHEMA_VERSION,
        });
    }

    let mut applied = Vec::new();
    while current < SCHEMA_VERSION {
        let m = MIGRATIONS
            .iter()
            .find(|m| m.from == current)
            .ok_or_else(|| Error::Config(format!("no migration from schema v{current}")))?;
        (m.apply)(&mut doc);
        applied.push(format!("schema v{current} → v{}", current + 1));
        current += 1;
    }

    // Always normalize the recorded schema version to the binary's.
    set_meta_int(&mut doc, "schema_version", SCHEMA_VERSION as i64);

    Ok((doc.to_string(), applied))
}

/// Set `[meta] framework_version` to `version`, preserving the rest of the file.
pub fn set_framework_version(text: &str, version: &str) -> Result<String> {
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| Error::Config(format!("meta.toml: {e}")))?;
    ensure_meta_table(&mut doc);
    doc["meta"]["framework_version"] = value(version);
    Ok(doc.to_string())
}

fn read_schema_version(doc: &DocumentMut) -> u32 {
    doc.get("meta")
        .and_then(|m| m.get("schema_version"))
        .and_then(|v| v.as_integer())
        .map(|i| i as u32)
        .unwrap_or(1)
}

fn ensure_meta_table(doc: &mut DocumentMut) {
    if doc.get("meta").is_none() {
        doc["meta"] = Item::Table(Default::default());
    }
}

fn set_meta_int(doc: &mut DocumentMut, key: &str, v: i64) {
    ensure_meta_table(doc);
    doc["meta"][key] = value(v);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_v1_is_noop_preserving_comments() {
        let src = "# my comment\n[meta]\nschema_version = 1\nprofile = \"rust\"\n";
        let (out, applied) = migrate(src).unwrap();
        assert!(applied.is_empty());
        assert!(out.contains("# my comment"));
        assert!(out.contains("schema_version = 1"));
    }

    #[test]
    fn migrate_sets_missing_schema_version() {
        let src = "[meta]\nprofile = \"rust\"\n";
        let (out, _) = migrate(src).unwrap();
        assert!(out.contains("schema_version = 1"));
        assert!(out.contains("profile = \"rust\""));
    }

    #[test]
    fn rejects_future_schema() {
        let src = "[meta]\nschema_version = 999\n";
        assert!(matches!(migrate(src), Err(Error::SchemaTooNew { .. })));
    }

    #[test]
    fn set_framework_version_preserves_user_content() {
        let src = "[meta]\nschema_version = 1\nframework_version = \"0.1.0\"\n[project]\ntitle = \"x\"\n# keep me\n";
        let out = set_framework_version(src, "0.2.0").unwrap();
        assert!(out.contains("framework_version = \"0.2.0\""));
        assert!(out.contains("# keep me"));
        assert!(out.contains("title = \"x\""));
    }
}
