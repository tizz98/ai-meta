//! Unit tests for the guard engine (see `guards.rs`).
//! Split out so `guards.rs` stays under the oversized-file threshold.

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
    let r = run(
        GuardId::NoPanicInLib,
        &ctx(tmp.path(), ProfileKind::Rust, &exts),
    );
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
    let r = run(
        GuardId::NoPanicInLib,
        &ctx(tmp.path(), ProfileKind::Rust, &exts),
    );
    match r {
        GuardResult::Trip { hits, .. } => {
            assert_eq!(hits.len(), 1);
            assert!(hits[0].contains("src/lib.rs:1"));
        }
        other => panic!("expected trip, got {other:?}"),
    }
}

#[test]
fn is_allowed_matches_only_the_named_guard() {
    let line = "x.unwrap(); // meta-allow: no-panic-in-lib";
    assert!(is_allowed(line, "no-panic-in-lib"));
    assert!(!is_allowed(line, "no-dbg-in-lib"));
    assert!(!is_allowed("x.unwrap();", "no-panic-in-lib"));
    // Multiple ids on one marker.
    assert!(is_allowed(
        "y // meta-allow: no-dbg-in-lib, no-panic-in-lib",
        "no-panic-in-lib"
    ));
    // A substring of the id must not count as a match.
    assert!(!is_allowed("z // meta-allow: panic", "no-panic-in-lib"));
}

#[test]
fn inline_allow_suppresses_only_marked_lines() {
    let tmp = tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("src")).unwrap();
    fs::write(
        tmp.path().join("src/lib.rs"),
        "fn a() { x.unwrap(); } // meta-allow: no-panic-in-lib\nfn b() { y.unwrap(); }\n",
    )
    .unwrap();
    let exts = vec!["rs".to_string()];
    match run(
        GuardId::NoPanicInLib,
        &ctx(tmp.path(), ProfileKind::Rust, &exts),
    ) {
        GuardResult::Trip { hits, .. } => {
            assert_eq!(hits.len(), 1);
            assert!(hits[0].contains("src/lib.rs:2"));
        }
        other => panic!("expected trip, got {other:?}"),
    }
}

#[test]
fn inline_allow_works_with_any_comment_style() {
    // The marker is matched by its text, independent of comment syntax.
    assert!(is_allowed(
        "x = y # meta-allow: no-panic-in-lib",
        "no-panic-in-lib"
    ));
    assert!(is_allowed(
        "foo() /* meta-allow: no-panic-in-lib */",
        "no-panic-in-lib"
    ));
    assert!(is_allowed(
        "<x> <!-- meta-allow: no-console-log -->",
        "no-console-log"
    ));
}

#[test]
fn strict_tsconfig_pass_and_fail() {
    let tmp = tempdir().unwrap();
    let exts = vec!["ts".to_string()];
    fs::write(
        tmp.path().join("tsconfig.json"),
        r#"{ "compilerOptions": { "strict": true } }"#,
    )
    .unwrap();
    assert!(matches!(
        run(
            GuardId::StrictTsconfig,
            &ctx(tmp.path(), ProfileKind::TypeScript, &exts)
        ),
        GuardResult::Pass(_)
    ));
    fs::write(
        tmp.path().join("tsconfig.json"),
        r#"{ "compilerOptions": { } }"#,
    )
    .unwrap();
    assert!(matches!(
        run(
            GuardId::StrictTsconfig,
            &ctx(tmp.path(), ProfileKind::TypeScript, &exts)
        ),
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
        run(
            GuardId::NoConsoleLog,
            &ctx(tmp.path(), ProfileKind::TypeScript, &exts)
        ),
        GuardResult::Pass(_)
    ));
    fs::write(tmp.path().join("src/bot/x.ts"), "console.log('leak')\n").unwrap();
    assert!(matches!(
        run(
            GuardId::NoConsoleLog,
            &ctx(tmp.path(), ProfileKind::TypeScript, &exts)
        ),
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
    fs::write(
        tmp.path().join("docs/dependencies.md"),
        "sketchy: needed for X",
    )
    .unwrap();
    assert!(matches!(
        run(GuardId::DepsJustified, &c),
        GuardResult::Pass(_)
    ));
}

#[test]
fn deps_justified_swift_from_package_manifest() {
    let tmp = tempdir().unwrap();
    let exts = vec!["swift".to_string()];
    fs::write(
        tmp.path().join("Package.swift"),
        r#"// swift-tools-version:5.9
import PackageDescription
let package = Package(
    name: "App",
    dependencies: [
        .package(url: "https://github.com/apple/swift-argument-parser.git", from: "1.0.0"),
        .package(url: "https://github.com/pointfreeco/swift-snapshot-testing", from: "1.0.0"),
        .package(path: "../LocalPkg"),
        // .package(url: "https://github.com/evil/Removed.git", from: "1.0.0"),
    ]
)
"#,
    )
    .unwrap();
    let allow = vec!["swift-argument-parser".to_string()];
    let c = GuardCtx {
        root: tmp.path(),
        profile: ProfileKind::Swift,
        source_exts: &exts,
        deps_allowlist: &allow,
        deps_doc: Some("docs/dependencies.md"),
    };
    match run(GuardId::DepsJustified, &c) {
        GuardResult::Trip { hits, .. } => {
            // Allowlisted arg-parser passes; local `.package(path:)` is internal;
            // only the undocumented snapshot-testing dependency trips.
            assert_eq!(hits.len(), 1);
            assert!(hits[0].contains("swift-snapshot-testing"));
        }
        other => panic!("expected trip, got {other:?}"),
    }
}

#[test]
fn no_print_in_lib_excludes_swiftpm_tests() {
    let tmp = tempdir().unwrap();
    let exts = vec!["swift".to_string()];
    fs::create_dir_all(tmp.path().join("Sources/App")).unwrap();
    fs::create_dir_all(tmp.path().join("Tests/AppTests")).unwrap();
    // A print() in a SwiftPM test file must NOT trip (Tests/ is test code).
    fs::write(
        tmp.path().join("Tests/AppTests/AppTests.swift"),
        "func testThing() { print(\"debug\") }\n",
    )
    .unwrap();
    assert!(matches!(
        run(
            GuardId::NoPrintInLib,
            &ctx(tmp.path(), ProfileKind::Swift, &exts)
        ),
        GuardResult::Pass(_)
    ));
    // A print() in library code under Sources/ still trips.
    fs::write(
        tmp.path().join("Sources/App/App.swift"),
        "func run() { print(\"leak\") }\n",
    )
    .unwrap();
    assert!(matches!(
        run(
            GuardId::NoPrintInLib,
            &ctx(tmp.path(), ProfileKind::Swift, &exts)
        ),
        GuardResult::Trip { .. }
    ));
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
