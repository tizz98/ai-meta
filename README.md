# ai-meta

The LLM's missing **meta framework** — one versioned Rust CLI (`meta`) that
scaffolds a project's tooling, enforces codified standards, talks to GitHub
natively, and keeps every consuming repo in sync as the framework evolves.

It replaces the copy-pasted bash `./meta` that otherwise drifts across repos:
the framework lives in one binary, projects carry only a tiny `.meta/meta.toml`
and a ~40-line `./meta` shim that fetches the pinned binary.

## Install

One-time setup — installs the `meta` CLI onto your PATH:

```
curl -fsSL https://raw.githubusercontent.com/tizz98/ai-meta/main/install.sh | bash
```

This fetches the latest release binary (checksum-verified) into
`~/.local/bin`. Override with `AI_META_VERSION` (pin a version) or
`AI_META_BIN_DIR` (install location). Then run `meta init` in a repo.

## What it does

- **`meta init`** — scaffold `.meta/meta.toml`, the `./meta` shim, GitHub Actions
  workflows, `CLAUDE.md` (with a managed block), `.claude/skills/meta-*`, and
  `META.md`. Auto-detects the language **profile** (rust / typescript / python /
  generic) and infers build/test/lint commands + domains from the repo. Uses the
  `claude` CLI to tailor wording when available (deterministic fallback otherwise).
- **`meta check` / `meta arch`** — a data-driven rule engine (guards + advisory
  architecture signals) with per-profile defaults, tunable thresholds, and custom
  grep guards. Runs on a plain box, no toolchain.
- **`meta build` / `test` / `gen` / `ci`** — run the profile/config commands;
  `ci` mirrors the gates and posts a collapsed PR comment.
- **`meta setup` / `task` / `milestone` / `wave` / `status`** — GitHub structure
  (labels, milestones, Projects v2) and issue-based task tracking via octocrab.
- **`meta upgrade`** — regenerate managed artifacts and migrate `meta.toml` to a
  newer framework version, preserving user-owned content; `--dry-run` shows a diff.
- **`meta tag`** — cut a release (version bump across configured locations, commit,
  tag, push).

## Config

Everything is a small `.meta/meta.toml`; omitted values inherit the baked
profile. See `META.md` and `docs/` for details.

To silence a single guard hit, add an inline `meta-allow: <guard-id>` marker in
a comment on the offending line — any comment style works (`// meta-allow:
no-panic-in-lib`, `# meta-allow: no-print-in-lib`, `/* … */`). List several ids
comma-separated; the marker only suppresses the guards it names.

## Develop

```
cargo build           # build the `meta` binary
cargo test            # unit + integration tests
cargo run -- check    # dogfood the standards on this repo
```
