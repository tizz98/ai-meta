# CLAUDE.md — ai-meta

ai-meta is the framework itself: a single Rust binary (`meta`) that scaffolds,
lints, and syncs project tooling across repos. It is self-hosting — it manages
its own `.meta/meta.toml` and dogfoods its own `check`/`arch`.

<!-- meta:managed:start -->
## Commands (via the `./meta` CLI)

This project is managed by [ai-meta](https://github.com/tizz98/ai-meta). During
development, run the CLI with `cargo run -- <cmd>` (e.g. `cargo run -- check`).

- Build: `cargo build` · Test: `cargo test`
- Lint: `cargo run -- check` — codified standards (advisory in CI)
- Architecture review: `cargo run -- arch`
- Format + clippy: `cargo fmt --all --check` · `cargo clippy --all-targets -- -D warnings`

Config lives in `.meta/meta.toml`; profile defaults are baked into the binary.

## Architecture (module map)

- `cli` — clap command tree + dispatch.
- `config` — meta.toml schema, profile-merge into `EffectiveConfig`, migrations.
- `profile` — baked per-language defaults (rust/typescript/python/generic).
- `detect` — profile + command inference for an existing repo.
- `rules` — the engine: `grep` scanner, `nontest` `#[cfg(test)]` balancer,
  `guards`, `signals`, `coverage`.
- `template` + `scaffold` + `assets/` — the renderer and managed-artifact set.
- `sync` — upgrade plan/apply, diff, version compat; `claudegen` — optional AI wording.
- `github` — octocrab REST + Projects v2 GraphQL.
- `commands/*` — one module per subcommand.

## Before finishing a change

- `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- The rule engine is the core promise — any change to `rules/nontest` or
  `rules/guards` needs tests (it's security-relevant: a wrong exclusion hides
  real findings).
- New dependencies must be justified in `docs/dependencies.md`.
<!-- meta:managed:end -->
