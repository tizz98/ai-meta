//! Unified-diff rendering for `upgrade --dry-run`, via the `similar` crate.

use crate::output;
use similar::{ChangeTag, TextDiff};

/// Render a colored unified diff of `old` → `new`. Returns an empty string when
/// they are identical.
pub fn unified(old: &str, new: &str) -> String {
    if old == new {
        return String::new();
    }
    let diff = TextDiff::from_lines(old, new);
    let mut out = String::new();
    for change in diff.iter_all_changes() {
        let line = change.value();
        let rendered = match change.tag() {
            ChangeTag::Delete => output::red(&format!("-{}", line.trim_end())),
            ChangeTag::Insert => output::green(&format!("+{}", line.trim_end())),
            ChangeTag::Equal => continue,
        };
        out.push_str(&rendered);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_is_empty() {
        assert_eq!(unified("a\nb\n", "a\nb\n"), "");
    }

    #[test]
    fn shows_added_and_removed() {
        crate::output::set_color(false);
        let d = unified("a\nb\n", "a\nc\n");
        assert!(d.contains("-b"));
        assert!(d.contains("+c"));
    }
}
