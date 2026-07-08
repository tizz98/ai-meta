//! Faithful Rust port of the bash `_nontest_grep` awk balancer.
//!
//! A `#[cfg(test)]` attribute applies to the single item that follows it. We
//! skip that item so test-only `unwrap()`/`panic!` doesn't trip the panic guard,
//! WITHOUT a real Rust parser:
//!   - block items (`mod tests { … }`, `fn helper() { … }`) → skip to the
//!     matching closing brace (brace-depth tracking);
//!   - statement items (`use …;`) → skip just to the terminating `;`.
//!
//! Crucially, a braceless `#[cfg(test)] use …;` must NOT latch the skip onto the
//! production code that follows it — the documented bug that let production
//! `unwrap()` evade the guard. Before counting braces/semicolons (and before
//! matching the guard pattern) we strip string literals, char literals, and
//! `// line comments` so neither a brace nor the searched-for pattern inside a
//! literal counts as real code. String state is carried **across lines**, so a
//! multi-line string or raw string (e.g. a `let src = "…"` test fixture holding
//! example `unwrap()`s) is fully ignored. Block comments (`/* … */`) remain a
//! known heuristic limitation.

use regex::Regex;
use std::sync::OnceLock;

fn cfg_test_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // A hardcoded static pattern; a compile failure is a programmer bug to
    // surface immediately, not a runtime condition.
    RE.get_or_init(|| Regex::new(r"#\[cfg\(test\)\]").expect("static regex")) // meta-allow: no-panic-in-lib
}

/// The `async` keyword as a standalone word, opening an `async fn`, `async {}`,
/// or `async move {}` scope. `\basync\b` deliberately does NOT match
/// `async_trait` (a `_` follows, so there is no word boundary), so the common
/// `#[async_trait]` attribute doesn't spuriously open a scope.
fn async_kw_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\basync\b").expect("static regex")) // meta-allow: no-panic-in-lib
}

/// Tracks whether the scanner is inside a string literal that opened on an
/// earlier line, so multi-line strings can be stripped across line boundaries.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lit {
    /// Real code (not inside any string literal).
    Code,
    /// Inside a normal `"…"` string left open by a previous line.
    Str,
    /// Inside a raw `r#"…"#` string (with the recorded `#` count) left open.
    Raw(usize),
}

/// Strip literal interiors and `//` line comments from a line so brace/semicolon
/// counting and pattern matching only see real code structure. `state` carries
/// an unterminated string from the previous line; the returned [`Lit`] is the
/// state to feed into the next line. Handles normal strings (`"…\"…"`), char
/// literals (`'…'`), and Rust raw strings (`r"…"`, `r#"…"#`, any hash count),
/// all of which may span multiple lines. Code braces/semicolons are kept;
/// literal contents and delimiters are dropped. Block comments (`/* … */`) are
/// the remaining known limitation.
fn strip(line: &str, state: Lit) -> (String, Lit) {
    let b = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;

    // Finish a string left open by the previous line before scanning code.
    match state {
        Lit::Code => {}
        Lit::Str => match consume_str(b, 0) {
            Some(end) => i = end,
            None => return (out, Lit::Str),
        },
        Lit::Raw(hashes) => match consume_raw(b, 0, hashes) {
            Some(end) => i = end,
            None => return (out, Lit::Raw(hashes)),
        },
    }

    while i < b.len() {
        let c = b[i];
        // Line comment: rest of the line is dropped.
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            break;
        }
        // Raw string: r, then zero+ '#', then '"' … '"' + same '#' count.
        if c == b'r' {
            let mut j = i + 1;
            let mut hashes = 0;
            while j < b.len() && b[j] == b'#' {
                hashes += 1;
                j += 1;
            }
            if j < b.len() && b[j] == b'"' {
                match consume_raw(b, j + 1, hashes) {
                    Some(end) => {
                        i = end;
                        continue;
                    }
                    None => return (out, Lit::Raw(hashes)),
                }
            }
        }
        // Normal string literal with backslash escapes (may run past EOL).
        if c == b'"' {
            match consume_str(b, i + 1) {
                Some(end) => {
                    i = end;
                    continue;
                }
                None => return (out, Lit::Str),
            }
        }
        // Char literal vs. lifetime. A `'` opens a char literal only when it is
        // an escape (`'\n'`, `'\''`) or a single char closed by another `'`
        // (`'x'`). A Rust lifetime (`'static`, `'a`) also starts with `'` but is
        // NOT closed by a quote — treating it as a char literal would consume to
        // the next quote (often end-of-line), swallowing the `{`/`}`/`;` that the
        // cfg(test)-skip balancer relies on and desyncing it, so test-only code
        // gets flagged as production. Only enter char-literal mode for the two
        // real forms; emit a lifetime tick as ordinary code and keep scanning.
        if c == b'\'' {
            let is_escape = i + 1 < b.len() && b[i + 1] == b'\\';
            let is_simple_char = i + 2 < b.len() && b[i + 2] == b'\'';
            if is_escape || is_simple_char {
                i += 1;
                while i < b.len() {
                    if b[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if b[i] == b'\'' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            // Lifetime (or a stray quote): treat as code so braces after it count.
            out.push('\'');
            i += 1;
            continue;
        }
        out.push(c as char);
        i += 1;
    }
    (out, Lit::Code)
}

/// Consume a normal string body from `from` (just past the opening `"`). Returns
/// the index just past the closing `"`, or `None` if the string runs to EOL
/// (a multi-line string continuing on the next line).
fn consume_str(b: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

/// Consume a raw-string body that started after the opening quote at `from`,
/// closing on `"` followed by `hashes` `#`. Returns the index just past the
/// close, or `None` if it runs to EOL (continuing on the next line).
fn consume_raw(b: &[u8], from: usize, hashes: usize) -> Option<usize> {
    let mut i = from;
    while i < b.len() {
        if b[i] == b'"' {
            let mut k = i + 1;
            let mut seen = 0;
            while k < b.len() && seen < hashes && b[k] == b'#' {
                seen += 1;
                k += 1;
            }
            if seen == hashes {
                return Some(k);
            }
        }
        i += 1;
    }
    None
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Normal,
    AwaitingItem,
    InBlock,
}

/// Advance the skip state machine over one (already stripped) line. Used both
/// for the `#[cfg(test)]` attribute line itself and for the lines that follow it
/// while skipping. (The bash `next`s past the attribute line without counting
/// its braces — we process it too, which fixes a false-negative for the
/// same-line `#[cfg(test)] mod t { … }` form while leaving the standard
/// own-line form identical, since `#[cfg(test)]` alone has no `{`/`;`.)
fn advance(line: &str, skip: &mut bool, phase: &mut Phase, depth: &mut i64) {
    match *phase {
        Phase::AwaitingItem => {
            let b = line.find('{');
            let s = line.find(';');
            if b.is_none() && s.is_none() {
                // blank / comment / attribute / signature continuation
                return;
            }
            let block_begins = match (b, s) {
                (None, _) => false,
                (Some(_), None) => true,
                (Some(bp), Some(sp)) => bp < sp,
            };
            if block_begins {
                *phase = Phase::InBlock;
                let n = line.matches('{').count() as i64;
                let m = line.matches('}').count() as i64;
                *depth = n - m;
                if *depth <= 0 {
                    *skip = false;
                    *phase = Phase::Normal;
                }
            } else {
                // statement item ends here (the `;`)
                *skip = false;
                *phase = Phase::Normal;
            }
        }
        Phase::InBlock => {
            let n = line.matches('{').count() as i64;
            let m = line.matches('}').count() as i64;
            *depth += n - m;
            if *depth <= 0 {
                *skip = false;
                *phase = Phase::Normal;
            }
        }
        Phase::Normal => {}
    }
}

/// Return (1-based line, line text) for lines matching `re`, EXCLUDING any line
/// attributed to a `#[cfg(test)]` item.
pub fn rust_nontest_match_lines(text: &str, re: &Regex) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut skip = false;
    let mut phase = Phase::Normal;
    let mut depth: i64 = 0;
    let mut lit = Lit::Code;

    for (idx, raw) in text.lines().enumerate() {
        let nr = idx + 1;
        // Match, attribute-detect, and brace-count on the stripped line so that
        // string-literal and `//` comment contents never count as real code. A
        // `#[cfg(test)]` mention inside a string or comment must NOT latch the
        // skip (that would hide the production code after it), and a pattern
        // that only appears inside a literal/comment is not a real use. `lit`
        // carries an unterminated string across lines so multi-line literals
        // (e.g. test fixtures) are ignored in full.
        let (stripped, next_lit) = strip(raw, lit);
        lit = next_lit;

        if cfg_test_re().is_match(&stripped) {
            skip = true;
            phase = Phase::AwaitingItem;
            depth = 0;
            advance(&stripped, &mut skip, &mut phase, &mut depth);
            continue;
        }

        if skip {
            advance(&stripped, &mut skip, &mut phase, &mut depth);
            continue;
        }

        if re.is_match(&stripped) {
            out.push((nr, raw.to_string()));
        }
    }
    out
}

/// Like [`rust_nontest_match_lines`], but a hit must ALSO sit inside an
/// `async fn` / `async {}` / `async move {}` scope. Used by
/// `no-blocking-in-async`, whose premise — "blocking call in async code" — only
/// holds inside async scopes; synchronous helpers doing `std::fs` are correct
/// and must not be flagged.
///
/// Async-scope tracking is a heuristic sibling of the `#[cfg(test)]` balancer:
/// it counts braces in production (non-test) code and remembers the depths at
/// which async bodies opened, reporting a line only while inside one. It is
/// deliberately biased toward the safe direction for a linter — a scope it fails
/// to recognize (e.g. a hand-written `-> impl Future` with no `async` keyword)
/// merely drops a warning, it never invents one. Known limits: a blocking call
/// inside a `spawn_blocking`/`block_in_place` closure nested in an async fn still
/// reads as "in async" (suppress those per-line with `meta-allow`), and a bare
/// `async fn foo();` trait declaration is prevented from latching onto the next
/// body by canceling the pending scope at its terminating `;`.
pub fn rust_async_blocking_match_lines(text: &str, re: &Regex) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut skip = false;
    let mut phase = Phase::Normal;
    let mut depth: i64 = 0;
    let mut lit = Lit::Code;

    // Production-code brace depth and the depths at which open async bodies live.
    let mut gdepth: i64 = 0;
    let mut async_bodies: Vec<i64> = Vec::new();
    // Seen an `async` opener whose body `{` has not arrived yet (multi-line
    // signatures put the brace on a later line).
    let mut pending_async = false;

    for (idx, raw) in text.lines().enumerate() {
        let nr = idx + 1;
        let (stripped, next_lit) = strip(raw, lit);
        lit = next_lit;

        if cfg_test_re().is_match(&stripped) {
            skip = true;
            phase = Phase::AwaitingItem;
            depth = 0;
            advance(&stripped, &mut skip, &mut phase, &mut depth);
            continue;
        }
        if skip {
            advance(&stripped, &mut skip, &mut phase, &mut depth);
            continue;
        }

        let in_async_before = !async_bodies.is_empty();
        if async_kw_re().is_match(&stripped) {
            pending_async = true;
        }

        // Walk the line's braces to keep `gdepth` and the async-body stack in
        // sync; `entered` catches a one-line `async fn … { … }` whose scope opens
        // and closes on the same line as the blocking call.
        let mut entered = false;
        for ch in stripped.bytes() {
            match ch {
                b'{' => {
                    gdepth += 1;
                    if pending_async {
                        async_bodies.push(gdepth);
                        pending_async = false;
                        entered = true;
                    }
                }
                b'}' => {
                    if async_bodies.last() == Some(&gdepth) {
                        async_bodies.pop();
                    }
                    if gdepth > 0 {
                        gdepth -= 1;
                    }
                }
                // A `;` before any body brace ends a bodyless `async fn foo();`
                // declaration — don't let it latch onto the next fn's body.
                b';' if pending_async => pending_async = false,
                _ => {}
            }
        }

        if (in_async_before || entered) && re.is_match(&stripped) {
            out.push((nr, raw.to_string()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panic_re() -> Regex {
        Regex::new(r"\.unwrap\(\)|\.expect\(|panic!\(|unreachable!\(").unwrap()
    }

    #[test]
    fn flags_production_unwrap() {
        let src = "fn run() {\n    let x = thing.unwrap();\n}\n";
        let hits = rust_nontest_match_lines(src, &panic_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 2);
    }

    #[test]
    fn skips_unwrap_inside_cfg_test_module() {
        let src = "\
#[cfg(test)]
mod tests {
    fn t() {
        thing.unwrap();
    }
}
fn prod() {
    other.unwrap();
}
";
        let hits = rust_nontest_match_lines(src, &panic_re());
        // Only the production unwrap on line 8 should be flagged.
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 8);
    }

    #[test]
    fn braceless_cfg_test_use_does_not_latch_onto_production() {
        // The documented bug: a braceless `#[cfg(test)] use …;` must skip ONLY
        // that statement, not the production unwrap that follows.
        let src = "\
#[cfg(test)]
use crate::testutil::*;

fn prod() {
    value.unwrap();
}
";
        let hits = rust_nontest_match_lines(src, &panic_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 5);
    }

    #[test]
    fn brace_in_string_literal_does_not_unbalance() {
        // `const OPEN: &str = "{";` inside the test block must not leave the
        // skipper latched (the strip() guarantee).
        let src = "\
#[cfg(test)]
mod tests {
    const OPEN: &str = \"{\";
    fn t() { x.unwrap(); }
}
fn prod() { y.unwrap(); }
";
        let hits = rust_nontest_match_lines(src, &panic_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 6);
    }

    #[test]
    fn same_line_cfg_test_block() {
        let src = "\
#[cfg(test)] mod t { fn a() { z.unwrap(); } }
fn prod() { w.unwrap(); }
";
        let hits = rust_nontest_match_lines(src, &panic_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 2);
    }

    #[test]
    fn raw_string_braces_do_not_unbalance() {
        // A raw string containing unbalanced braces inside the test block must
        // not leak the skip onto the following production code.
        let src = "\
#[cfg(test)]
mod tests {
    fn t() {
        let j = r#\"{ \"a\": { } \"#;
        x.unwrap();
    }
}
fn prod() { y.unwrap(); }
";
        let hits = rust_nontest_match_lines(src, &panic_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 8);
    }

    #[test]
    fn pattern_inside_string_or_comment_is_not_a_real_use() {
        // The engine greps its own rule-definition files; a pattern that only
        // appears inside a string literal or `//` comment (e.g. a guard's own
        // message text) must not be flagged as a production use. A synthetic
        // `PROBE(` token stands in for the real pattern so this fixture does not
        // self-match when `meta check` scans this very file.
        let re = Regex::new(r"PROBE\(").unwrap();
        let src = "\
let msg = \"PROBE( left in code\";
// remember: forbid PROBE( everywhere
fn prod() { PROBE(x); }
";
        let hits = rust_nontest_match_lines(src, &re);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 3);
    }

    #[test]
    fn multiline_string_fixture_is_not_real_code() {
        // A multi-line string literal holding code-shaped text (a test fixture)
        // must be stripped across line boundaries, so the `unwrap()` inside it is
        // not flagged — while real code after the string still is.
        let src = "\
fn build() {
    let _fixture = \"
fn prod() {
    x.unwrap();
}
\";
}
fn real() {
    y.unwrap();
}
";
        let hits = rust_nontest_match_lines(src, &panic_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 9);
    }

    #[test]
    fn cfg_test_in_comment_does_not_skip_production() {
        // A `#[cfg(test)]` mention inside a comment must NOT start the test-skip
        // — otherwise the production code that follows evades the panic guard
        // (a false-negative, the dangerous direction).
        let src = "\
// see #[cfg(test)]
fn prod() { value.unwrap(); }
";
        let hits = rust_nontest_match_lines(src, &panic_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 2);
    }

    #[test]
    fn nested_braces_in_test_block() {
        let src = "\
#[cfg(test)]
mod tests {
    fn t() {
        if true {
            for _ in 0..1 {
                a.unwrap();
            }
        }
    }
}
fn prod() { b.expect(\"x\"); }
";
        let hits = rust_nontest_match_lines(src, &panic_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 11);
    }

    #[test]
    fn lifetime_does_not_swallow_test_block_brace() {
        // A `'static` (or any lifetime) on the block-opening line must NOT be
        // mistaken for a char literal — otherwise the char scanner eats to the
        // next quote / end of line, swallowing the `{` that opens the fn body,
        // the balancer desyncs, and the cfg(test) skip drops early so test-only
        // `unwrap()`s get flagged as production. Only the real production
        // unwrap on the last line should be flagged.
        let src = "\
#[cfg(test)]
mod tests {
    fn boxed() -> Box<dyn Iterator<Item = ()> + 'static> {
        thing.unwrap();
    }
    fn t() { other.unwrap(); }
}
fn prod() { value.unwrap(); }
";
        let hits = rust_nontest_match_lines(src, &panic_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 8);
    }

    #[test]
    fn char_literal_with_brace_is_still_stripped() {
        // The disambiguation must not regress real char literals: a `'{'` inside
        // the test block must still be stripped so its brace doesn't unbalance
        // the skip and leak onto the following production code.
        let src = "\
#[cfg(test)]
mod tests {
    fn t() {
        let open = '{';
        x.unwrap();
    }
}
fn prod() { y.unwrap(); }
";
        let hits = rust_nontest_match_lines(src, &panic_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 8);
    }

    fn blocking_re() -> Regex {
        Regex::new(r"std::thread::sleep|std::fs::(read|write|File::)").unwrap()
    }

    #[test]
    fn blocking_flagged_inside_async_fn_only() {
        // The same `std::fs::read` is a problem in the async fn and fine in the
        // synchronous helper — only the async one is reported.
        let src = "\
async fn load() {
    let _ = std::fs::read(\"a\");
}
fn load_sync() {
    let _ = std::fs::read(\"b\");
}
";
        let hits = rust_async_blocking_match_lines(src, &blocking_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 2);
    }

    #[test]
    fn blocking_in_multiline_async_signature_body() {
        // The body brace lands on a later line than the `async fn` keyword; the
        // scanner must still know it is inside the async scope.
        let src = "\
async fn load(
    path: &str,
) -> Vec<u8> {
    std::fs::read(path).unwrap_or_default()
}
";
        let hits = rust_async_blocking_match_lines(src, &blocking_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 4);
    }

    #[test]
    fn blocking_in_async_block_is_flagged() {
        let src = "\
fn spawn() {
    tokio::spawn(async move {
        std::fs::write(\"x\", b\"y\").ok();
    });
}
";
        let hits = rust_async_blocking_match_lines(src, &blocking_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 3);
    }

    #[test]
    fn blocking_in_cfg_test_async_is_skipped() {
        let src = "\
#[cfg(test)]
mod tests {
    async fn t() {
        std::fs::read(\"a\").unwrap();
    }
}
";
        let hits = rust_async_blocking_match_lines(src, &blocking_re());
        assert!(hits.is_empty());
    }

    #[test]
    fn bodyless_async_fn_decl_does_not_latch_onto_sync_fn() {
        // A trait's `async fn` declaration has no body; its pending scope must be
        // cancelled at the `;` so the following synchronous fn is not mistaken
        // for async code.
        let src = "\
trait Loader {
    async fn load(&self);
}
fn helper() {
    let _ = std::fs::read(\"a\");
}
";
        let hits = rust_async_blocking_match_lines(src, &blocking_re());
        assert!(hits.is_empty());
    }

    #[test]
    fn one_line_async_fn_with_blocking_is_flagged() {
        let src = "async fn go() { std::fs::read(\"a\").ok(); }\n";
        let hits = rust_async_blocking_match_lines(src, &blocking_re());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, 1);
    }
}
