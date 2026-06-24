//! Running external commands (build/test/lint/codegen). Commands are stored as
//! strings in config; we tokenize them quote-aware, check the program is on
//! PATH (so a missing toolchain degrades to a clean SKIP rather than an error),
//! and run them either inheriting the terminal or capturing output.

use std::path::Path;
use std::process::{Command, Stdio};

/// Captured output of a command.
#[derive(Debug, Clone)]
pub struct Output {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Tokenize a command string into argv, honoring single/double quotes (so an
/// arg like `--ignore '(a|b)'` survives). Backslash escapes the next char.
pub fn split_args(s: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut started = false;

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
                started = true;
            }
            '"' if !in_single => {
                in_double = !in_double;
                started = true;
            }
            '\\' if !in_single => {
                if let Some(next) = chars.next() {
                    cur.push(next);
                    started = true;
                }
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if started {
                    args.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            c => {
                cur.push(c);
                started = true;
            }
        }
    }
    if started {
        args.push(cur);
    }
    args
}

/// Is `program` resolvable on PATH (or an existing executable path)?
pub fn which(program: &str) -> bool {
    if program.contains('/') {
        return Path::new(program).exists();
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(program);
            if candidate.is_file() {
                return true;
            }
        }
    }
    false
}

/// The program name a command string starts with, if any.
pub fn program_of(cmd: &str) -> Option<String> {
    split_args(cmd).into_iter().next()
}

/// Run a command inheriting stdio. Returns the exit code (or 127 if the program
/// could not be spawned). `cwd` is the working directory.
pub fn run_inherited(cmd: &str, cwd: &Path) -> i32 {
    let argv = split_args(cmd);
    if argv.is_empty() {
        return 0;
    }
    let status = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .status();
    match status {
        Ok(s) => s.code().unwrap_or(1),
        Err(_) => 127,
    }
}

/// Run a command capturing stdout/stderr.
pub fn run_captured(cmd: &str, cwd: &Path) -> std::io::Result<Output> {
    let argv = split_args(cmd);
    if argv.is_empty() {
        return Ok(Output {
            status: 0,
            stdout: String::new(),
            stderr: String::new(),
        });
    }
    let out = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()?;
    Ok(Output {
        status: out.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_plain() {
        assert_eq!(
            split_args("cargo build --workspace"),
            vec!["cargo", "build", "--workspace"]
        );
    }

    #[test]
    fn splits_quoted() {
        assert_eq!(
            split_args("cargo llvm-cov --ignore '(tests/|sdk/)'"),
            vec!["cargo", "llvm-cov", "--ignore", "(tests/|sdk/)"]
        );
        assert_eq!(
            split_args(r#"sh -c "echo hi there""#),
            vec!["sh", "-c", "echo hi there"]
        );
    }

    #[test]
    fn empty_is_empty() {
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn which_finds_sh() {
        assert!(which("sh"));
        assert!(!which("definitely-not-a-real-program-xyz"));
    }

    #[test]
    fn captures_output() {
        let tmp = tempfile::tempdir().unwrap();
        let out = run_captured("echo hello", tmp.path()).unwrap();
        assert_eq!(out.status, 0);
        assert!(out.stdout.contains("hello"));
    }
}
