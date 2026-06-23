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
//! `unwrap()` evade the guard. Before counting braces/semicolons we strip string
//! literals, char literals, and `// line comments` so a brace inside a literal
//! can't unbalance the count. Block comments (`/* { */`) and raw strings
//! (`r#"{"#`) remain a known heuristic limitation, exactly as in the bash.

use regex::Regex;
use std::sync::OnceLock;

fn cfg_test_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"#\[cfg\(test\)\]").expect("static regex"))
}

fn string_lit_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#""([^"\\]|\\.)*""#).expect("static regex"))
}

fn char_lit_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"'[^']*'").expect("static regex"))
}

/// Strip string literals, char literals, and `//` line comments from a line —
/// the `strip()` awk function — so brace/semicolon counting can't be fooled.
fn strip(line: &str) -> String {
    let s = string_lit_re().replace_all(line, "");
    let s = char_lit_re().replace_all(&s, "");
    match s.find("//") {
        Some(i) => s[..i].to_string(),
        None => s.to_string(),
    }
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

    for (idx, raw) in text.lines().enumerate() {
        let nr = idx + 1;

        // A `#[cfg(test)]` attribute (checked on the RAW line) resets the
        // skip state, then we also process the remainder of this line.
        if cfg_test_re().is_match(raw) {
            skip = true;
            phase = Phase::AwaitingItem;
            depth = 0;
            advance(&strip(raw), &mut skip, &mut phase, &mut depth);
            continue;
        }

        if skip {
            advance(&strip(raw), &mut skip, &mut phase, &mut depth);
            continue;
        }

        if re.is_match(raw) {
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
}
