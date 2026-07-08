# Releasing

Cutting a release publishes two artifacts from a single `v*` tag:

1. **GitHub Release binaries** — cross-compiled `meta` binaries + checksums for
   linux (musl) and macOS, consumed by `install.sh` / the `./meta` shim.
2. **crates.io** — the `ai-meta` crate, installable with `cargo install ai-meta`
   (the binary is named `meta`).

Both are driven by `.github/workflows/release.yml`.

## Steps

1. `cargo run -- tag` (or `./meta tag`) — bumps the version across the configured
   locations (`Cargo.toml`), commits, tags `vX.Y.Z`, and pushes.
2. The `release` workflow fires on the pushed tag:
   - `build` cross-compiles every target and uploads binaries to the release.
   - `publish-crate` runs **after** every build succeeds (crates.io releases are
     permanent), verifies the tag matches the crate version, then runs
     `cargo publish --locked`.

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
