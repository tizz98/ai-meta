//! A deliberately tiny template engine — just enough for the embedded scaffold
//! assets (workflows, shim, docs). Supports `{{ var }}` substitution and
//! non-nested `{{#if flag}} … {{/if}}` blocks. Lists are pre-rendered in Rust
//! and injected as a single variable, so the engine stays ~1 screen and adds no
//! dependency (a full handlebars would be overkill here).

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// Rendering context: string variables + boolean flags.
#[derive(Debug, Default, Clone)]
pub struct Ctx {
    vars: HashMap<String, String>,
    flags: HashSet<String>,
}

impl Ctx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn var(mut self, key: &str, value: impl Into<String>) -> Self {
        self.vars.insert(key.to_string(), value.into());
        self
    }

    pub fn flag(mut self, key: &str, on: bool) -> Self {
        if on {
            self.flags.insert(key.to_string());
        }
        self
    }

    pub fn set_var(&mut self, key: &str, value: impl Into<String>) {
        self.vars.insert(key.to_string(), value.into());
    }
}

fn if_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)\{\{#if\s+(\w+)\}\}(.*?)\{\{/if\}\}").expect("static regex"))
}

fn var_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\{\{\s*(\w+)\s*\}\}").expect("static regex"))
}

/// Render `template` against `ctx`. `{{#if}}` blocks are resolved first (so a
/// dropped block's variables never need values), then `{{var}}` substitution.
pub fn render(template: &str, ctx: &Ctx) -> String {
    let after_if = if_re().replace_all(template, |caps: &regex::Captures| {
        let flag = &caps[1];
        if ctx.flags.contains(flag) {
            caps[2].to_string()
        } else {
            String::new()
        }
    });
    var_re()
        .replace_all(&after_if, |caps: &regex::Captures| {
            ctx.vars.get(&caps[1]).cloned().unwrap_or_default()
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_vars() {
        let ctx = Ctx::new().var("name", "realtime-rs");
        assert_eq!(render("hi {{ name }}!", &ctx), "hi realtime-rs!");
    }

    #[test]
    fn unknown_var_is_empty() {
        assert_eq!(render("a{{missing}}b", &Ctx::new()), "ab");
    }

    #[test]
    fn if_block_included_and_excluded() {
        let on = Ctx::new().flag("cov", true).var("min", "80");
        assert_eq!(
            render("x{{#if cov}} gate {{min}}{{/if}}y", &on),
            "x gate 80y"
        );
        let off = Ctx::new();
        assert_eq!(render("x{{#if cov}} gate{{/if}}y", &off), "xy");
    }

    #[test]
    fn multiple_if_blocks() {
        let ctx = Ctx::new().flag("a", true);
        assert_eq!(render("{{#if a}}A{{/if}}-{{#if b}}B{{/if}}", &ctx), "A-");
    }
}
