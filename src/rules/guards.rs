//! Built-in guard implementations. A guard inspects the working tree and
//! returns a [`GuardResult`] (Skip / Pass / Trip). The trip *severity* is
//! supplied by config at run time (see [`super::run_checks`]) — the guard only
//! decides whether it tripped and surfaces the offending hits.

use super::grep::{self, Hit};
use super::model::{CustomGuard, GuardId};
use crate::profile::ProfileKind;
use regex::Regex;
use std::collections::BTreeSet;
use std::path::Path;

/// What a guard found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardResult {
    /// Nothing to inspect (pre-scaffold) — a clean, non-failing outcome.
    Skip(String),
    /// Inspected and clean.
    Pass(String),
    /// Tripped: `summary` + offending `hits` (rendered `file:line:text`).
    Trip { summary: String, hits: Vec<String> },
}

/// Inputs a guard needs.
pub struct GuardCtx<'a> {
    pub root: &'a Path,
    pub profile: ProfileKind,
    pub source_exts: &'a [String],
    pub deps_allowlist: &'a [String],
    pub deps_doc: Option<&'a str>,
}

impl GuardCtx<'_> {
    fn has_sources(&self) -> bool {
        !grep::collect_files(self.root, self.source_exts, &[]).is_empty()
    }
}

/// Run a built-in guard.
pub fn run(id: GuardId, ctx: &GuardCtx) -> GuardResult {
    match id {
        GuardId::NoPanicInLib => nontest_scan(
            ctx,
            r"\.unwrap\(\)|\.expect\(|panic!\(|unreachable!\(",
            "no unwrap/expect/panic! in non-test code",
            "force-panic ops (unwrap/expect/panic!/unreachable!) in non-test code — prefer typed errors / graceful degradation",
        ),
        GuardId::NoBlockingInAsync => nontest_scan(
            ctx,
            r"std::thread::sleep|std::fs::(read|write|File::)|reqwest::blocking",
            "no blocking std calls in async code",
            "blocking std call in async code — use the async equivalents (tokio::time::sleep, tokio::fs)",
        ),
        GuardId::NoDbgInLib => nontest_scan(
            ctx,
            r"\bdbg!\(",
            "no dbg!() left in non-test code",
            "dbg!() macro left in non-test code — remove debug prints before committing",
        ),
        GuardId::StrictTsconfig => strict_tsconfig(ctx),
        GuardId::NoDebugger => simple_scan(
            ctx,
            r"(^|[^.\w])debugger\s*;",
            "no 'debugger' statements",
            "stray 'debugger' statement(s) committed — remove before merging",
            true,
        ),
        GuardId::NoTsIgnore => simple_scan(
            ctx,
            r"@ts-ignore",
            "no @ts-ignore (expect-error preferred)",
            "@ts-ignore found — prefer '@ts-expect-error <reason>' so stale suppressions fail the build",
            false,
        ),
        GuardId::NoConsoleLog => console_log(ctx),
        GuardId::NoPrintInLib => print_in_lib(ctx),
        GuardId::NoBareExcept => simple_scan(
            ctx,
            r"except\s*:",
            "no bare 'except:' clauses",
            "bare 'except:' swallows everything — catch a specific exception type",
            true,
        ),
        GuardId::NoFocusedTests => no_focused_tests(ctx),
        GuardId::DepsJustified => deps_justified(ctx),
    }
}

/// Run a project-defined custom grep guard from meta.toml.
pub fn run_custom(c: &CustomGuard, ctx: &GuardCtx) -> GuardResult {
    let re = match Regex::new(&c.pattern) {
        Ok(re) => re,
        Err(e) => {
            return GuardResult::Trip {
                summary: format!("custom guard {:?} has an invalid pattern: {e}", c.name),
                hits: vec![],
            }
        }
    };
    if !ctx.has_sources() {
        return GuardResult::Skip("no sources yet".into());
    }
    let hits = scan(ctx, &re, &c.roots, c.exclude_tests);
    if hits.is_empty() {
        GuardResult::Pass(format!("{}: clean", c.name))
    } else {
        let summary = if c.message.is_empty() {
            format!("{}: matches found", c.name)
        } else {
            c.message.clone()
        };
        GuardResult::Trip {
            summary,
            hits: render(hits),
        }
    }
}

// --- guard implementations ---------------------------------------------------

fn nontest_scan(ctx: &GuardCtx, pattern: &str, pass: &str, trip: &str) -> GuardResult {
    if !ctx.has_sources() {
        return GuardResult::Skip("no sources yet".into());
    }
    let re = compile(pattern);
    let hits = scan(ctx, &re, &[], true);
    finish(hits, pass, trip)
}

fn simple_scan(
    ctx: &GuardCtx,
    pattern: &str,
    pass: &str,
    trip: &str,
    exclude_tests: bool,
) -> GuardResult {
    if !ctx.has_sources() {
        return GuardResult::Skip("no sources yet".into());
    }
    let re = compile(pattern);
    let hits = scan(ctx, &re, &[], exclude_tests);
    finish(hits, pass, trip)
}

fn strict_tsconfig(ctx: &GuardCtx) -> GuardResult {
    let path = ctx.root.join("tsconfig.json");
    if !path.is_file() {
        return GuardResult::Skip("no tsconfig.json yet".into());
    }
    let text = grep::read(&path);
    let re = compile(r#""strict"\s*:\s*true"#);
    if re.is_match(&text) {
        GuardResult::Pass(r#""strict": true in tsconfig.json"#.into())
    } else {
        GuardResult::Trip {
            summary: r#"tsconfig.json must set "strict": true"#.into(),
            hits: vec![],
        }
    }
}

fn console_log(ctx: &GuardCtx) -> GuardResult {
    if !ctx.has_sources() {
        return GuardResult::Skip("no sources yet".into());
    }
    let re = compile(r"console\.log\(");
    let hits: Vec<Hit> = scan(ctx, &re, &[], false)
        .into_iter()
        .filter(|h| !h.file.contains("src/cli/") && !h.file.starts_with("cli/"))
        .collect();
    finish(
        hits,
        "no stray console.log outside src/cli/",
        "console.log outside src/cli/ — route diagnostics through a logger",
    )
}

fn print_in_lib(ctx: &GuardCtx) -> GuardResult {
    if !ctx.has_sources() {
        return GuardResult::Skip("no sources yet".into());
    }
    let re = compile(r"\bprint\(");
    let hits: Vec<Hit> = scan(ctx, &re, &[], true)
        .into_iter()
        .filter(|h| !h.file.contains("/cli/") && !h.file.starts_with("cli/") && !h.file.ends_with("__main__.py"))
        .collect();
    finish(
        hits,
        "no stray print() in library code",
        "print() in library code — route diagnostics through a logger (cli/ may print)",
    )
}

fn no_focused_tests(ctx: &GuardCtx) -> GuardResult {
    // Only meaningful for JS/TS test files (`.only(`).
    if ctx.profile != ProfileKind::TypeScript {
        return GuardResult::Skip("no focused-test concept for this profile".into());
    }
    let test_files: Vec<_> = grep::collect_files(ctx.root, ctx.source_exts, &[])
        .into_iter()
        .filter(|p| {
            let rel = grep::rel_display(ctx.root, p);
            rel.contains(".test.") || rel.contains(".spec.")
        })
        .collect();
    if test_files.is_empty() {
        return GuardResult::Skip("no test files yet".into());
    }
    let re = compile(r"\b(describe|it|test|context)\.only\(");
    let mut hits = Vec::new();
    for f in &test_files {
        let rel = grep::rel_display(ctx.root, f);
        for (line, text) in grep::match_lines(&grep::read(f), &re) {
            hits.push(Hit { file: rel.clone(), line, text });
        }
    }
    finish(
        hits,
        "no focused (.only) tests",
        "focused test(s) committed (.only) — they hide the rest of the suite",
    )
}

fn deps_justified(ctx: &GuardCtx) -> GuardResult {
    let declared = match ctx.profile {
        ProfileKind::Rust => rust_deps(ctx.root),
        ProfileKind::TypeScript => ts_deps(ctx.root),
        ProfileKind::Python => py_deps(ctx.root),
        ProfileKind::Generic => return GuardResult::Skip("no dependency model for generic".into()),
    };
    let declared = match declared {
        Some(d) => d,
        None => return GuardResult::Skip("no manifest yet".into()),
    };
    if declared.is_empty() {
        return GuardResult::Pass("no third-party dependencies declared".into());
    }
    let doc_text = ctx
        .deps_doc
        .map(|d| grep::read(&ctx.root.join(d)))
        .unwrap_or_default();
    let allow: BTreeSet<&str> = ctx.deps_allowlist.iter().map(|s| s.as_str()).collect();

    let mut undocumented = Vec::new();
    for (manifest, dep) in &declared {
        if allow.contains(dep.as_str()) {
            continue;
        }
        if !doc_text.is_empty() && doc_text.contains(dep.as_str()) {
            continue;
        }
        undocumented.push(format!("{manifest}: {dep}"));
    }
    if undocumented.is_empty() {
        GuardResult::Pass("all dependencies allowlisted or documented".into())
    } else {
        GuardResult::Trip {
            summary: "dependency not in the allowlist and not justified in the deps doc".into(),
            hits: undocumented,
        }
    }
}

// --- dependency extraction ---------------------------------------------------

/// (manifest-rel-path, dep-name) pairs from all Cargo.toml files.
fn rust_deps(root: &Path) -> Option<Vec<(String, String)>> {
    let manifests = grep::collect_files(root, &["toml".into()], &[]);
    let manifests: Vec<_> = manifests
        .into_iter()
        .filter(|p| p.file_name().map(|n| n == "Cargo.toml").unwrap_or(false))
        .collect();
    if manifests.is_empty() {
        return None;
    }
    let dep_line = compile(r"^[A-Za-z0-9_-]+\s*=");
    let section = compile(r"^\[(dependencies|dev-dependencies|build-dependencies|workspace\.dependencies)\]");
    let any_section = compile(r"^\[");
    let mut out = Vec::new();
    for m in &manifests {
        let rel = grep::rel_display(root, m);
        let text = grep::read(m);
        let mut in_deps = false;
        for line in text.lines() {
            let trimmed = line.trim_start();
            if section.is_match(trimmed) {
                in_deps = true;
                continue;
            }
            if any_section.is_match(trimmed) {
                in_deps = false;
                continue;
            }
            if in_deps && dep_line.is_match(trimmed) {
                let name = trimmed.split([' ', '=', '.']).next().unwrap_or("").to_string();
                // workspace-inherited (`name.workspace = true`) and internal crates exempt.
                if line.contains(".workspace") {
                    continue;
                }
                if !name.is_empty() {
                    out.push((rel.clone(), name));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    Some(out)
}

/// Runtime `dependencies` from package.json (devDependencies are exempt).
fn ts_deps(root: &Path) -> Option<Vec<(String, String)>> {
    let path = root.join("package.json");
    if !path.is_file() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&grep::read(&path)).ok()?;
    let mut out = Vec::new();
    if let Some(obj) = v.get("dependencies").and_then(|d| d.as_object()) {
        for k in obj.keys() {
            out.push(("package.json".to_string(), k.clone()));
        }
    }
    out.sort();
    Some(out)
}

/// Dependencies from pyproject.toml `[project] dependencies` or requirements.txt.
fn py_deps(root: &Path) -> Option<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut found_manifest = false;

    let pyproject = root.join("pyproject.toml");
    if pyproject.is_file() {
        found_manifest = true;
        if let Ok(v) = grep::read(&pyproject).parse::<toml::Value>() {
            if let Some(deps) = v
                .get("project")
                .and_then(|p| p.get("dependencies"))
                .and_then(|d| d.as_array())
            {
                for d in deps {
                    if let Some(s) = d.as_str() {
                        out.push(("pyproject.toml".to_string(), py_name(s)));
                    }
                }
            }
        }
    }
    let req = root.join("requirements.txt");
    if req.is_file() {
        found_manifest = true;
        for line in grep::read(&req).lines() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            out.push(("requirements.txt".to_string(), py_name(l)));
        }
    }
    if !found_manifest {
        return None;
    }
    out.sort();
    out.dedup();
    Some(out)
}

/// Strip a PEP 508 requirement to its bare distribution name.
fn py_name(spec: &str) -> String {
    spec.split(|c: char| "<>=!~[ ;(".contains(c))
        .next()
        .unwrap_or(spec)
        .trim()
        .to_string()
}

// --- shared helpers ----------------------------------------------------------

fn scan(ctx: &GuardCtx, re: &Regex, roots: &[String], exclude_tests: bool) -> Vec<Hit> {
    let is_rust = ctx.profile == ProfileKind::Rust;
    let mut out = Vec::new();
    for path in grep::collect_files(ctx.root, ctx.source_exts, roots) {
        let rel = grep::rel_display(ctx.root, &path);
        if exclude_tests && grep::is_test_path(&rel, ctx.source_exts) {
            continue;
        }
        let text = grep::read(&path);
        let matched = if exclude_tests && is_rust {
            super::nontest::rust_nontest_match_lines(&text, re)
        } else {
            grep::match_lines(&text, re)
        };
        for (line, text) in matched {
            out.push(Hit { file: rel.clone(), line, text });
        }
    }
    out
}

fn finish(hits: Vec<Hit>, pass: &str, trip: &str) -> GuardResult {
    if hits.is_empty() {
        GuardResult::Pass(pass.into())
    } else {
        GuardResult::Trip {
            summary: trip.into(),
            hits: render(hits),
        }
    }
}

fn render(hits: Vec<Hit>) -> Vec<String> {
    hits.iter().map(Hit::render).collect()
}

fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("built-in guard pattern is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn ctx<'a>(root: &'a Path, profile: ProfileKind, exts: &'a [String]) -> GuardCtx<'a> {
        GuardCtx {
            root,
            profile,
            source_exts: exts,
            deps_allowlist: &[],
            deps_doc: Some("docs/dependencies.md"),
        }
    }

    #[test]
    fn panic_guard_skips_when_no_sources() {
        let tmp = tempdir().unwrap();
        let exts = vec!["rs".to_string()];
        let r = run(GuardId::NoPanicInLib, &ctx(tmp.path(), ProfileKind::Rust, &exts));
        assert!(matches!(r, GuardResult::Skip(_)));
    }

    #[test]
    fn panic_guard_trips_on_production_unwrap_only() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(
            tmp.path().join("src/lib.rs"),
            "fn p(){ a.unwrap(); }\n#[cfg(test)]\nmod t { fn x(){ b.unwrap(); } }\n",
        )
        .unwrap();
        let exts = vec!["rs".to_string()];
        let r = run(GuardId::NoPanicInLib, &ctx(tmp.path(), ProfileKind::Rust, &exts));
        match r {
            GuardResult::Trip { hits, .. } => {
                assert_eq!(hits.len(), 1);
                assert!(hits[0].contains("src/lib.rs:1"));
            }
            other => panic!("expected trip, got {other:?}"),
        }
    }

    #[test]
    fn strict_tsconfig_pass_and_fail() {
        let tmp = tempdir().unwrap();
        let exts = vec!["ts".to_string()];
        fs::write(tmp.path().join("tsconfig.json"), r#"{ "compilerOptions": { "strict": true } }"#).unwrap();
        assert!(matches!(
            run(GuardId::StrictTsconfig, &ctx(tmp.path(), ProfileKind::TypeScript, &exts)),
            GuardResult::Pass(_)
        ));
        fs::write(tmp.path().join("tsconfig.json"), r#"{ "compilerOptions": { } }"#).unwrap();
        assert!(matches!(
            run(GuardId::StrictTsconfig, &ctx(tmp.path(), ProfileKind::TypeScript, &exts)),
            GuardResult::Trip { .. }
        ));
    }

    #[test]
    fn console_log_allows_cli() {
        let tmp = tempdir().unwrap();
        let exts = vec!["ts".to_string()];
        fs::create_dir_all(tmp.path().join("src/cli")).unwrap();
        fs::create_dir_all(tmp.path().join("src/bot")).unwrap();
        fs::write(tmp.path().join("src/cli/main.ts"), "console.log('ok')\n").unwrap();
        fs::write(tmp.path().join("src/bot/x.ts"), "let a=1;\n").unwrap();
        assert!(matches!(
            run(GuardId::NoConsoleLog, &ctx(tmp.path(), ProfileKind::TypeScript, &exts)),
            GuardResult::Pass(_)
        ));
        fs::write(tmp.path().join("src/bot/x.ts"), "console.log('leak')\n").unwrap();
        assert!(matches!(
            run(GuardId::NoConsoleLog, &ctx(tmp.path(), ProfileKind::TypeScript, &exts)),
            GuardResult::Trip { .. }
        ));
    }

    #[test]
    fn deps_justified_rust_allowlist_and_doc() {
        let tmp = tempdir().unwrap();
        let exts = vec!["rs".to_string()];
        fs::write(
            tmp.path().join("Cargo.toml"),
            "[dependencies]\ntokio = \"1\"\nsketchy = \"0.1\"\n",
        )
        .unwrap();
        let allow = vec!["tokio".to_string()];
        let c = GuardCtx {
            root: tmp.path(),
            profile: ProfileKind::Rust,
            source_exts: &exts,
            deps_allowlist: &allow,
            deps_doc: Some("docs/dependencies.md"),
        };
        match run(GuardId::DepsJustified, &c) {
            GuardResult::Trip { hits, .. } => {
                assert_eq!(hits.len(), 1);
                assert!(hits[0].contains("sketchy"));
            }
            other => panic!("expected trip, got {other:?}"),
        }
        // Document it -> pass.
        fs::create_dir_all(tmp.path().join("docs")).unwrap();
        fs::write(tmp.path().join("docs/dependencies.md"), "sketchy: needed for X").unwrap();
        assert!(matches!(run(GuardId::DepsJustified, &c), GuardResult::Pass(_)));
    }

    #[test]
    fn custom_guard_trips() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("core")).unwrap();
        fs::write(tmp.path().join("core/x.rs"), "use rdkafka::Foo;\n").unwrap();
        let exts = vec!["rs".to_string()];
        let c = CustomGuard {
            name: "no-rdkafka".into(),
            pattern: "use rdkafka".into(),
            roots: vec!["core".into()],
            exclude_tests: true,
            severity: super::super::model::Severity::Fail,
            message: "Backends only through the trait.".into(),
        };
        let r = run_custom(&c, &ctx(tmp.path(), ProfileKind::Rust, &exts));
        assert!(matches!(r, GuardResult::Trip { .. }));
    }
}
