# Dependencies

ai-meta keeps a lean, vetted dependency set. Each runtime/dev dependency is
justified here (the `deps-justified` guard checks this file).

## Runtime

- **clap** — argument parsing / the command tree and help.
- **anyhow** — ergonomic error context in command bodies.
- **thiserror** — typed library errors (`src/error.rs`).
- **serde** / **serde_json** — (de)serialize `meta.toml`, parse `package.json`,
  coverage JSON, and the GitHub API payloads.
- **toml** — parse `meta.toml`.
- **toml_edit** — format-preserving migration of the user-owned `meta.toml`.
- **regex** — the rule-engine scanner, the nontest balancer, version rewriting.
- **walkdir** — source-file enumeration for the rule engine.
- **similar** — unified diffs for `upgrade --dry-run`.
- **octocrab** — native GitHub REST + GraphQL (replaces shelling out to `gh`).
- **tokio** — async runtime octocrab requires (current-thread, used only by the
  GitHub-touching commands).

## Dev

- **tempfile** — temp dirs for unit/integration tests.
- **assert_cmd** / **predicates** — drive the built binary in integration tests.
