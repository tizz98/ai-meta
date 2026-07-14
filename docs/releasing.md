# Releasing

Cutting a release publishes two artifacts from a single `v*` tag:

1. **GitHub Release binaries** — cross-compiled `meta` binaries + checksums for
   linux (musl) and macOS, consumed by `install.sh` / the `./meta` shim.
2. **crates.io** — the `ai-meta` crate, installable with `cargo install ai-meta`
   (the binary is named `meta`).

## The version is the single source of truth

`Cargo.toml`'s `[package] version` drives everything. **You never create the tag
locally** — neither by hand nor via `meta tag` (which runs in bump-only mode
here). `release.yml` derives and pushes the tag from the merged version. Cutting
a `vX.Y.Z` tag yourself is exactly how a release fails at the "verify tag matches
crate version" step, or no-ops because the tag already exists.

## Steps

1. On a branch, run `cargo run -- tag <level>` (this repo runs `meta tag` in
   **bump-only** mode — see `.meta/meta.toml`). It rewrites `Cargo.toml` + the
   `Cargo.lock` entry, commits `chore: release vX.Y.Z`, and pushes the branch —
   but does **not** create the tag, because `release.yml` owns tagging. Open a PR
   from that branch. `cargo run -- tag <level> --dry-run` previews without
   touching anything.
2. Merge to `main`. The `release` workflow fires on the merge push and runs three
   jobs **in one run**:
   - `prepare` reads the new version and pushes the matching `vX.Y.Z` tag (skips
     if the tag already exists, e.g. a Cargo.toml edit that didn't bump).
   - `build` cross-compiles every target from the tag and uploads binaries.
   - `publish-crate` runs **after** every build succeeds (crates.io releases are
     permanent) and runs `cargo publish --locked`.

To (re)ship the current version manually — or if a run needs re-triggering — use
the workflow's **Run workflow** button (`workflow_dispatch`); it releases the
version currently in `Cargo.toml`.

## Why tag creation and release live in one workflow

A tag pushed by a workflow using the default `GITHUB_TOKEN` does **not** trigger
another workflow — GitHub suppresses it to prevent recursive runs. So a split
design (one workflow auto-tags, a second releases `on: push: tags`) silently
never releases: the release workflow's tag trigger is exactly the suppressed
event. Folding tag creation and the release into a single run — triggered by the
human merge push, which *does* trigger workflows — avoids that trap.

## Guardrails (why a mismatch can't ship)

- The tag is **derived from** `Cargo.toml` in `prepare`, so the tag and crate
  version can't disagree, and `build`/`publish-crate` check out that exact tag.
- **`lockfile.yml`** fails any PR whose `Cargo.lock` drifts from `Cargo.toml`, so
  `cargo publish --locked` never breaks on a stale lockfile.
- `publish-crate` re-verifies the checked-out crate version before uploading.

## One-time setup

- Add a crates.io API token as the `CARGO_REGISTRY_TOKEN` repository secret
  (Settings → Secrets and variables → Actions). Create the token at
  <https://crates.io/settings/tokens> scoped to `publish-update` (and
  `publish-new` for the first release).
- The crate name `ai-meta` must be owned by the publishing account. The first
  `cargo publish` claims it; subsequent releases just push new versions.

## Notes

- The published crate excludes repo-management artifacts (`.github/`, `.claude/`,
  `.meta/`, `docs/`, `install.sh`, the `./meta` shim) via `exclude` in
  `Cargo.toml` — see that list to keep it in sync.
- A version can only be published once. If `publish-crate` fails after the crate
  is live (e.g. a transient error), re-running will fail with "already uploaded";
  bump to a new patch version instead.
