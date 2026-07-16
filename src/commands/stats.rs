//! `meta stats` — read-only repo analytics: commit counts by author (`commits`)
//! and lines of code by language (`cloc`). Every subcommand supports `--json`
//! for machine consumption; the human form mirrors the other commands' output.

use crate::rules::grep;
use crate::{context, output, process};
use clap::{Args, Subcommand};
use serde::Serialize;
use std::path::Path;

#[derive(Args, Debug)]
pub struct StatsArgs {
    /// Emit machine-readable JSON instead of the human summary.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub sub: StatsCmd,
}

#[derive(Subcommand, Debug)]
pub enum StatsCmd {
    /// Commit counts by author (from `git log`).
    #[command(alias = "c")]
    Commits {
        /// Only count commits whose author name or email contains this
        /// (case-insensitive).
        #[arg(long)]
        user: Option<String>,
    },
    /// Lines of code by language.
    #[command(alias = "loc")]
    Cloc {
        /// Only count one language (by name or extension, case-insensitive).
        #[arg(long)]
        lang: Option<String>,
    },
}

/// One author's share of the log.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AuthorStat {
    pub name: String,
    pub email: String,
    pub commits: usize,
}

#[derive(Debug, Serialize)]
pub struct CommitStats {
    pub total: usize,
    pub authors: Vec<AuthorStat>,
}

/// One language's line counts (`code` = `lines` − `blank`).
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct LangStat {
    pub language: String,
    pub files: usize,
    pub lines: usize,
    pub blank: usize,
    pub code: usize,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct ClocTotal {
    pub files: usize,
    pub lines: usize,
    pub blank: usize,
    pub code: usize,
}

#[derive(Debug, Serialize)]
pub struct ClocStats {
    pub total: ClocTotal,
    pub languages: Vec<LangStat>,
}

pub fn run(args: StatsArgs) -> anyhow::Result<i32> {
    let root = context::require_root()?;
    match args.sub {
        StatsCmd::Commits { user } => run_commits(&root, user.as_deref(), args.json),
        StatsCmd::Cloc { lang } => run_cloc(&root, lang.as_deref(), args.json),
    }
}

fn run_commits(root: &Path, user: Option<&str>, json: bool) -> anyhow::Result<i32> {
    // %aN/%aE honor .mailmap; one tab-separated name/email pair per commit.
    let out = process::run_captured("git log --pretty=format:%aN%x09%aE", root)
        .map_err(|e| anyhow::anyhow!("failed to run git: {e}"))?;
    let log = if out.status == 0 {
        out.stdout
    } else if out.stderr.contains("does not have any commits") {
        String::new()
    } else {
        anyhow::bail!("git log failed: {}", out.stderr.trim());
    };

    let stats = aggregate_commits(&log, user);
    if json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(0);
    }
    match user {
        Some(u) => output::head(format!("Commits by author (matching '{u}')")),
        None => output::head("Commits by author"),
    }
    if stats.authors.is_empty() {
        output::info("  (none)");
    }
    for a in &stats.authors {
        println!("  {:>6}  {} <{}>", a.commits, a.name, a.email);
    }
    output::info(format!("\n{} commit(s) total", stats.total));
    Ok(0)
}

fn run_cloc(root: &Path, lang: Option<&str>, json: bool) -> anyhow::Result<i32> {
    let exts: Vec<String> = LANGUAGES.iter().map(|(e, _)| e.to_string()).collect();
    let entries: Vec<(String, usize, usize)> = grep::collect_files(root, &exts, &[])
        .iter()
        .filter_map(|p| {
            let language = language_of(p.extension()?.to_str()?)?;
            let (lines, blank) = count_lines(&grep::read(p));
            Some((language.to_string(), lines, blank))
        })
        .collect();

    let stats = aggregate_cloc(&entries, lang);
    if json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(0);
    }
    output::head("Lines of code");
    if stats.languages.is_empty() {
        output::info("  (none)");
        return Ok(0);
    }
    println!(
        "  {:<14} {:>6} {:>8} {:>8} {:>8}",
        "Language", "Files", "Lines", "Blank", "Code"
    );
    for l in &stats.languages {
        println!(
            "  {:<14} {:>6} {:>8} {:>8} {:>8}",
            l.language, l.files, l.lines, l.blank, l.code
        );
    }
    let t = &stats.total;
    println!(
        "  {:<14} {:>6} {:>8} {:>8} {:>8}",
        "Total", t.files, t.lines, t.blank, t.code
    );
    Ok(0)
}

/// Aggregate `git log` output (one `name\temail` pair per line) into per-author
/// counts, optionally keeping only authors whose name or email contains `user`
/// (case-insensitive). Sorted by commit count descending, then name.
pub fn aggregate_commits(log: &str, user: Option<&str>) -> CommitStats {
    let filter = user.map(|u| u.to_lowercase());
    let mut by_author: std::collections::BTreeMap<(String, String), usize> = Default::default();
    for line in log.lines() {
        let (name, email) = line.split_once('\t').unwrap_or((line, ""));
        if let Some(f) = &filter {
            if !name.to_lowercase().contains(f) && !email.to_lowercase().contains(f) {
                continue;
            }
        }
        *by_author
            .entry((name.to_string(), email.to_string()))
            .or_default() += 1;
    }
    let mut authors: Vec<AuthorStat> = by_author
        .into_iter()
        .map(|((name, email), commits)| AuthorStat {
            name,
            email,
            commits,
        })
        .collect();
    authors.sort_by(|a, b| b.commits.cmp(&a.commits).then(a.name.cmp(&b.name)));
    CommitStats {
        total: authors.iter().map(|a| a.commits).sum(),
        authors,
    }
}

/// Extension → language, for every language `cloc` counts.
const LANGUAGES: &[(&str, &str)] = &[
    ("rs", "Rust"),
    ("ts", "TypeScript"),
    ("tsx", "TypeScript"),
    ("js", "JavaScript"),
    ("jsx", "JavaScript"),
    ("mjs", "JavaScript"),
    ("cjs", "JavaScript"),
    ("py", "Python"),
    ("go", "Go"),
    ("java", "Java"),
    ("kt", "Kotlin"),
    ("c", "C"),
    ("cpp", "C++"),
    ("cc", "C++"),
    ("h", "C/C++ Header"),
    ("hpp", "C/C++ Header"),
    ("cs", "C#"),
    ("rb", "Ruby"),
    ("php", "PHP"),
    ("swift", "Swift"),
    ("sh", "Shell"),
    ("bash", "Shell"),
    ("ps1", "PowerShell"),
    ("sql", "SQL"),
    ("html", "HTML"),
    ("css", "CSS"),
    ("scss", "SCSS"),
    ("md", "Markdown"),
    ("toml", "TOML"),
    ("yaml", "YAML"),
    ("yml", "YAML"),
    ("json", "JSON"),
];

/// The language a file extension belongs to, if it's one we count.
pub fn language_of(ext: &str) -> Option<&'static str> {
    LANGUAGES
        .iter()
        .find(|(e, _)| ext.eq_ignore_ascii_case(e))
        .map(|(_, lang)| *lang)
}

/// `(total, blank)` line counts; a line is blank when whitespace-only.
pub fn count_lines(content: &str) -> (usize, usize) {
    let mut lines = 0;
    let mut blank = 0;
    for l in content.lines() {
        lines += 1;
        if l.trim().is_empty() {
            blank += 1;
        }
    }
    (lines, blank)
}

/// Does `language` match a `--lang` filter (its name, or one of its extensions)?
fn lang_matches(language: &str, filter: &str) -> bool {
    language.eq_ignore_ascii_case(filter) || language_of(filter) == Some(language)
}

/// Roll per-file `(language, lines, blank)` entries up into per-language stats,
/// optionally keeping only the language matching `lang` (name or extension,
/// case-insensitive). Sorted by code lines descending, then name.
pub fn aggregate_cloc(entries: &[(String, usize, usize)], lang: Option<&str>) -> ClocStats {
    let mut by_lang: std::collections::BTreeMap<&str, LangStat> = Default::default();
    for (language, lines, blank) in entries {
        if let Some(f) = lang {
            if !lang_matches(language, f) {
                continue;
            }
        }
        let s = by_lang.entry(language).or_insert_with(|| LangStat {
            language: language.clone(),
            files: 0,
            lines: 0,
            blank: 0,
            code: 0,
        });
        s.files += 1;
        s.lines += lines;
        s.blank += blank;
        s.code += lines - blank;
    }
    let mut languages: Vec<LangStat> = by_lang.into_values().collect();
    languages.sort_by(|a, b| b.code.cmp(&a.code).then(a.language.cmp(&b.language)));
    let mut total = ClocTotal::default();
    for l in &languages {
        total.files += l.files;
        total.lines += l.lines;
        total.blank += l.blank;
        total.code += l.code;
    }
    ClocStats { total, languages }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_and_sorts_authors() {
        let log = "Alice\talice@example.com\nBob\tbob@example.com\nAlice\talice@example.com\n";
        let s = aggregate_commits(log, None);
        assert_eq!(s.total, 3);
        assert_eq!(
            s.authors[0],
            AuthorStat {
                name: "Alice".into(),
                email: "alice@example.com".into(),
                commits: 2
            }
        );
        assert_eq!(s.authors[1].name, "Bob");
    }

    #[test]
    fn filters_by_user_name_or_email_case_insensitive() {
        let log = "Alice\talice@example.com\nBob\tbob@example.com\n";
        let by_name = aggregate_commits(log, Some("ALICE"));
        assert_eq!(by_name.total, 1);
        assert_eq!(by_name.authors.len(), 1);
        assert_eq!(by_name.authors[0].name, "Alice");

        let by_email = aggregate_commits(log, Some("bob@"));
        assert_eq!(by_email.authors.len(), 1);
        assert_eq!(by_email.authors[0].name, "Bob");
    }

    #[test]
    fn empty_log_is_zero_commits() {
        let s = aggregate_commits("", None);
        assert_eq!(s.total, 0);
        assert!(s.authors.is_empty());
    }

    #[test]
    fn maps_extensions_to_languages() {
        assert_eq!(language_of("rs"), Some("Rust"));
        assert_eq!(language_of("py"), Some("Python"));
        assert_eq!(language_of("tsx"), Some("TypeScript"));
        assert_eq!(language_of("bin"), None);
    }

    #[test]
    fn counts_total_and_blank_lines() {
        assert_eq!(count_lines("a\n\nb\n"), (3, 1));
        assert_eq!(count_lines("  \n"), (1, 1));
        assert_eq!(count_lines(""), (0, 0));
    }

    #[test]
    fn aggregates_cloc_sorted_by_code() {
        let entries = vec![
            ("Rust".to_string(), 3, 1),
            ("Rust".to_string(), 5, 0),
            ("Python".to_string(), 100, 10),
        ];
        let s = aggregate_cloc(&entries, None);
        assert_eq!(s.total.files, 3);
        assert_eq!(s.total.lines, 108);
        assert_eq!(s.languages[0].language, "Python");
        assert_eq!(
            s.languages[1],
            LangStat {
                language: "Rust".into(),
                files: 2,
                lines: 8,
                blank: 1,
                code: 7
            }
        );
    }

    #[test]
    fn cloc_filter_matches_name_or_extension() {
        let entries = vec![("Rust".to_string(), 3, 1), ("Python".to_string(), 2, 0)];
        let by_name = aggregate_cloc(&entries, Some("RUST"));
        assert_eq!(by_name.languages.len(), 1);
        assert_eq!(by_name.languages[0].language, "Rust");

        let by_ext = aggregate_cloc(&entries, Some("py"));
        assert_eq!(by_ext.languages.len(), 1);
        assert_eq!(by_ext.languages[0].language, "Python");
        assert_eq!(by_ext.total.lines, 2);
    }
}
