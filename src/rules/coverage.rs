//! Coverage-percentage parsing for the supported tools. Fails closed: a result
//! that isn't a number returns `None` so the CI gate never treats an unmeasured
//! run as green (mirrors the bash `_ci_coverage_pct` hardening).

/// Parse line-coverage percent from `cargo llvm-cov --json` output:
/// `{ "data": [ { "totals": { "lines": { "percent": <f64> } } } ] }`.
pub fn parse_llvm_cov(json: &str) -> Option<f64> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("data")?
        .get(0)?
        .get("totals")?
        .get("lines")?
        .get("percent")?
        .as_f64()
}

/// Parse line-coverage percent from a vitest/istanbul `coverage-summary.json`:
/// `{ "total": { "lines": { "pct": <f64> } } }`.
pub fn parse_vitest(json: &str) -> Option<f64> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("total")?.get("lines")?.get("pct")?.as_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llvm_cov_ok() {
        let j = r#"{ "data": [ { "totals": { "lines": { "percent": 83.33, "count": 100 } } } ] }"#;
        assert_eq!(parse_llvm_cov(j), Some(83.33));
    }

    #[test]
    fn vitest_ok() {
        let j = r#"{ "total": { "lines": { "pct": 80.0 }, "statements": { "pct": 75.0 } } }"#;
        assert_eq!(parse_vitest(j), Some(80.0));
    }

    #[test]
    fn fails_closed_on_garbage() {
        assert_eq!(parse_llvm_cov("not json"), None);
        assert_eq!(parse_vitest("{}"), None);
        assert_eq!(parse_llvm_cov(r#"{"data":[]}"#), None);
    }
}
