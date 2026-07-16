//! End-to-end `stats` tests: drive the real binary against temp repos and
//! assert the commit/cloc numbers (human and `--json` forms).

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;

fn meta(dir: &Path) -> Command {
    let mut c = Command::cargo_bin("meta").unwrap();
    c.current_dir(dir);
    c.env("CI", "1");
    c
}

/// Run git isolated from the user's global/system config (signing, mailmap…).
fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn commit(dir: &Path, name: &str, email: &str, file: &str) {
    fs::write(dir.join(file), file).unwrap();
    git(dir, &["add", "."]);
    git(
        dir,
        &[
            "-c",
            &format!("user.name={name}"),
            "-c",
            &format!("user.email={email}"),
            "commit",
            "-q",
            "-m",
            &format!("add {file}"),
            "--no-gpg-sign",
        ],
    );
}

/// A repo with 2 commits by Alice and 1 by Bob.
fn repo_with_commits() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    git(tmp.path(), &["init", "-q"]);
    commit(tmp.path(), "Alice", "alice@example.com", "a.txt");
    commit(tmp.path(), "Alice", "alice@example.com", "b.txt");
    commit(tmp.path(), "Bob", "bob@example.com", "c.txt");
    tmp
}

fn json_stdout(cmd: &mut Command) -> Value {
    let out = cmd.assert().success().get_output().stdout.clone();
    serde_json::from_slice(&out).expect("stdout is valid JSON")
}

#[test]
fn commits_json_counts_by_author() {
    let repo = repo_with_commits();
    let v = json_stdout(meta(repo.path()).args(["stats", "commits", "--json"]));

    assert_eq!(v["total"], 3);
    let authors = v["authors"].as_array().unwrap();
    assert_eq!(authors.len(), 2);
    // Sorted by commit count descending.
    assert_eq!(authors[0]["name"], "Alice");
    assert_eq!(authors[0]["email"], "alice@example.com");
    assert_eq!(authors[0]["commits"], 2);
    assert_eq!(authors[1]["name"], "Bob");
    assert_eq!(authors[1]["commits"], 1);
}

#[test]
fn commits_user_filter_via_alias() {
    let repo = repo_with_commits();
    // `c` is the alias; --user matches name or email, case-insensitively.
    let v = json_stdout(meta(repo.path()).args(["stats", "c", "--user", "BOB", "--json"]));

    assert_eq!(v["total"], 1);
    let authors = v["authors"].as_array().unwrap();
    assert_eq!(authors.len(), 1);
    assert_eq!(authors[0]["name"], "Bob");
}

#[test]
fn commits_human_output_lists_authors() {
    let repo = repo_with_commits();
    meta(repo.path())
        .args(["stats", "commits"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Alice"))
        .stdout(predicates::str::contains("3 commit(s)"));
}

/// A non-git-backed root (like init_test) with one Rust and one Python file.
fn repo_with_sources() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join(".git")).unwrap();
    fs::create_dir_all(tmp.path().join("src")).unwrap();
    // 3 lines, 1 blank.
    fs::write(tmp.path().join("src/main.rs"), "fn main() {\n\n}\n").unwrap();
    // 2 lines, 0 blank.
    fs::write(tmp.path().join("app.py"), "import os\nprint(os)\n").unwrap();
    tmp
}

#[test]
fn cloc_json_counts_lines_per_language() {
    let repo = repo_with_sources();
    let v = json_stdout(meta(repo.path()).args(["stats", "cloc", "--json"]));

    let langs = v["languages"].as_array().unwrap();
    let rust = langs
        .iter()
        .find(|l| l["language"] == "Rust")
        .expect("Rust reported");
    assert_eq!(rust["files"], 1);
    assert_eq!(rust["lines"], 3);
    assert_eq!(rust["blank"], 1);
    assert_eq!(rust["code"], 2);

    let python = langs
        .iter()
        .find(|l| l["language"] == "Python")
        .expect("Python reported");
    assert_eq!(python["files"], 1);
    assert_eq!(python["lines"], 2);
    assert_eq!(python["code"], 2);

    assert_eq!(v["total"]["files"], 2);
    assert_eq!(v["total"]["lines"], 5);
}

#[test]
fn cloc_lang_filter_via_alias() {
    let repo = repo_with_sources();
    // `loc` is the alias; --lang matches the language name case-insensitively.
    let v = json_stdout(meta(repo.path()).args(["stats", "loc", "--lang", "rust", "--json"]));

    let langs = v["languages"].as_array().unwrap();
    assert_eq!(langs.len(), 1);
    assert_eq!(langs[0]["language"], "Rust");
    assert_eq!(v["total"]["lines"], 3);
}

#[test]
fn cloc_human_output_lists_languages() {
    let repo = repo_with_sources();
    meta(repo.path())
        .args(["stats", "cloc"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Rust"))
        .stdout(predicates::str::contains("Python"));
}
