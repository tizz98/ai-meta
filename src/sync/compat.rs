//! Version-compatibility checks between a repo's recorded framework version and
//! the running binary. Cheap enough to run on every command.

use crate::output;
use crate::version::Version;

/// Emit a one-line NOTE when the repo is pinned behind/ahead of this binary.
/// Schema-too-new is handled as a hard error in `config::load`, not here.
pub fn runtime_note(recorded_framework: &str) {
    let recorded = match Version::parse(recorded_framework) {
        Ok(v) => v,
        Err(_) => return,
    };
    let binary = Version::framework();
    if binary > recorded {
        output::note(format!(
            "this repo pins ai-meta v{recorded} but v{binary} is running — run `meta upgrade` to update generated files."
        ));
    } else if recorded > binary {
        output::note(format!(
            "this repo pins ai-meta v{recorded}, newer than this binary (v{binary}) — update the pinned binary (.meta/version)."
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_garbage_without_panic() {
        runtime_note("not-a-version");
    }

    #[test]
    fn same_version_is_quiet() {
        // Smoke: a matching version must not panic (output goes to stderr).
        runtime_note(crate::version::FRAMEWORK_VERSION);
    }
}
