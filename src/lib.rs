//! ai-meta — the LLM's missing meta framework.
//!
//! One versioned CLI that scaffolds a project's tooling (`.meta/` config, the
//! `./meta` shim, GitHub Actions workflows, and agent docs), enforces codified
//! standards via a data-driven rule engine, talks to GitHub natively, and keeps
//! every consuming repo in sync as the framework evolves.
//!
//! The binary entry point is thin; everything testable lives here in the lib.

pub mod claudegen;
pub mod cli;
pub mod commands;
pub mod config;
pub mod context;
pub mod detect;
pub mod error;
pub mod github;
pub mod output;
pub mod process;
pub mod profile;
pub mod rules;
pub mod scaffold;
pub mod state;
pub mod sync;
pub mod template;
pub mod version;
