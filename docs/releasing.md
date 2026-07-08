# Releasing

Cutting a release publishes two artifacts from a single `v*` tag:

1. **GitHub Release binaries** — cross-compiled `meta` binaries + checksums for
   linux (musl) and macOS, consumed by `install.sh` / the `./meta` shim.
2. **crates.io** — the `ai-meta` crate, installable with `cargo install ai-meta`
   (the binary is named `meta`).

## The version is the single source of truth

`Cargo.toml`'s `[package] version` drives everything. **You never create a tag
by hand** — hand-cutting a `vX.Y.Z` tag without bumping the file is exactly how a
release fails at the "verify tag matches crate version" step.

## Steps

1. Open a PR that bumps `version` in `Cargo.toml` (and the matching `Cargo.lock`
   entry — `cargo build` updates it; the `lockfile` check enforces it). Locally,
   `cargo run -- tag <level> --dry-run` shows the bump without touching anything.
2. Merge to `main`. The `tag on version bump` workflow reads the new version and
   pushes the matching `vX.Y.Z` tag automatically — so the tag can never disagree
   with the file.
3. The `release` workflow fires on that tag:
   - `build` cross-compiles every target and uploads binaries to the release.
   - `publish-crate` runs **after** every build succeeds (crates.io releases are
     permanent), verifies the tag matches the crate version, then runs
     `cargo publish --locked`.

## Guardrails (why a mismatch can't ship)

- **`tag-on-version.yml`** derives the tag from `Cargo.toml`, so the normal path
  never produces a mismatched tag.
- **`lockfile.yml`** fails any PR whose `Cargo.lock` drifts from `Cargo.toml`, so
  `cargo publish --locked` never breaks on a stale lockfile.
- **`release.yml`**'s verify step is the backstop: if a tag is ever pushed by
  hand and doesn't match the crate version, the publish aborts before uploading.

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
