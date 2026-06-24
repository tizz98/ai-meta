# CLAUDE.md — ai-meta

ai-meta is the framework itself: a single Rust binary (`meta`) that scaffolds,
lints, and syncs project tooling across repos. It is self-hosting — it manages
its own `.meta/meta.toml` and dogfoods its own `check`/`arch`.

<!-- meta:managed:start -->
## Commands (via the `./meta` CLI)

This project is managed by [ai-meta](https://github.com/tizz98/ai-meta). The
`./meta` shim downloads the pinned binary (`.meta/version`) — no toolchain needed
just to run the standards checks.

- Build: `./meta build`  (`cargo build --workspace`)
- Test: `./meta test`  (`cargo test --workspace`)
- Lint: `./meta check` — codified standards (CI runs `--strict`)
- Architecture review (advisory): `./meta arch`
- Local CI (all gates): `./meta ci [PR]`
- Tasks: `./meta task` · Milestones: `./meta milestone` · Setup GitHub: `./meta setup`

Config lives in `.meta/meta.toml`; profile defaults are baked into the binary.
Run `./meta upgrade` to pull framework updates (workflows, skills, this block).

## Before finishing a change

- Run `./meta check` (and `./meta ci` before opening a PR).
- Don't hand-edit generated files (the `./meta` shim, `.github/workflows/*`,
  `.claude/skills/meta-*`, or this managed block) — change `.meta/meta.toml` and
  run `./meta upgrade`.
- Keep new dependencies justified (docs/dependencies.md) or on the allowlist.
<!-- meta:managed:end -->

> **Bootstrap note:** this repo *builds* the `meta` binary, so develop with
> `cargo run -- <cmd>` rather than the `./meta` shim — the shim downloads the
> pinned release, which only exists once a tag has been published.

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
- Don't hand-edit generated files (the `./meta` shim, `.github/workflows/*`,
  `.claude/skills/meta-*`, or the managed CLAUDE.md block) — change
  `.meta/meta.toml` and run `cargo run -- upgrade`.
