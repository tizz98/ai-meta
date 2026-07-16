//! End-to-end `init` tests: drive the real binary against temp repos and assert
//! the scaffolded files + detected profile.

use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn meta(dir: &Path) -> Command {
    let mut c = Command::cargo_bin("meta").unwrap();
    c.current_dir(dir);
    // Force determinism: never invoke the optional `claude` enrichment in tests.
    c.env("CI", "1");
    c
}

fn git_init(dir: &Path) {
    fs::create_dir_all(dir.join(".git")).unwrap();
}

#[test]
fn version_prints_schema() {
    let tmp = tempfile::tempdir().unwrap();
    meta(tmp.path())
        .arg("version")
        .assert()
        .success()
        .stdout(predicates::str::contains("config schema"));
}

#[test]
fn init_rust_workspace_scaffolds_expected_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git_init(root);
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"core\"]\n",
    )
    .unwrap();

    meta(root).args(["init", "--no-ai"]).assert().success();

    assert!(root.join(".meta/meta.toml").is_file());
    assert!(root.join(".meta/version").is_file());
    assert!(root.join("meta").is_file());
    assert!(root.join(".github/workflows/ci.yml").is_file());
    assert!(root.join(".github/workflows/meta-check.yml").is_file());
    assert!(root.join("CLAUDE.md").is_file());
    assert!(root.join(".claude/skills/meta-check/SKILL.md").is_file());
    assert!(root.join(".claude/skills/meta-stats/SKILL.md").is_file());

    let toml = fs::read_to_string(root.join(".meta/meta.toml")).unwrap();
    assert!(toml.contains("profile = \"rust\""));

    let ci = fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("dtolnay/rust-toolchain"));

    let claude = fs::read_to_string(root.join("CLAUDE.md")).unwrap();
    assert!(claude.contains("meta:managed:start"));
}

#[test]
fn init_autodetects_typescript_and_infers_commands() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git_init(root);
    fs::write(
        root.join("package.json"),
        r#"{ "version": "1.0.0", "scripts": { "build": "tsc", "typecheck": "tsc --noEmit" } }"#,
    )
    .unwrap();

    meta(root).args(["init", "--no-ai"]).assert().success();

    let toml = fs::read_to_string(root.join(".meta/meta.toml")).unwrap();
    assert!(toml.contains("profile = \"typescript\""));
    assert!(toml.contains("typecheck = \"npm run typecheck\""));
    let ci = fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("setup-node"));
}

#[test]
fn init_autodetects_python() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git_init(root);
    fs::write(
        root.join("pyproject.toml"),
        "[tool.ruff]\n[tool.pytest.ini_options]\n",
    )
    .unwrap();

    meta(root).args(["init", "--no-ai"]).assert().success();
    let toml = fs::read_to_string(root.join(".meta/meta.toml")).unwrap();
    assert!(toml.contains("profile = \"python\""));
    let ci = fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("setup-python"));
}

#[test]
fn init_dry_run_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git_init(root);
    fs::write(root.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();

    meta(root)
        .args(["init", "--no-ai", "--dry-run"])
        .assert()
        .success();
    assert!(!root.join(".meta/meta.toml").exists());
    assert!(!root.join("meta").exists());
}

#[test]
fn init_is_idempotent_for_meta_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git_init(root);
    fs::write(root.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();

    meta(root).args(["init", "--no-ai"]).assert().success();
    // Mark the user-owned file; a second init must not clobber it.
    let path = root.join(".meta/meta.toml");
    let mut content = fs::read_to_string(&path).unwrap();
    content.push_str("\n# user edit\n");
    fs::write(&path, &content).unwrap();

    meta(root).args(["init", "--no-ai"]).assert().success();
    assert!(fs::read_to_string(&path).unwrap().contains("# user edit"));
}

#[test]
fn check_runs_in_initialized_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git_init(root);
    fs::write(root.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
    meta(root).args(["init", "--no-ai"]).assert().success();
    meta(root).arg("check").assert().success();
}
