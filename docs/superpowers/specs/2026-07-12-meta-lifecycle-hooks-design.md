# Design: Configurable Lifecycle Hooks in `meta.toml`

Date: 2026-07-12
Status: Approved (design) — pending implementation plan

## Goal

Let a repo run its own shell commands around `meta` lifecycle events — e.g.
run the test suite before a release (`pre_tag`), notify or publish after one
(`post_tag`), or finish repo onboarding after `meta init` (`post_init`).
Hooks are declared in the user-owned `.meta/meta.toml`.

## Non-goals (YAGNI)

- Per-hook `optional`/`hard` failure override (fixed policy instead — see below).
- Shell features in the command string: `&&`, pipes, `$VAR` expansion, globs.
- Hook timeouts, parallel/async execution.
- Hooks on read-only commands (`status`, `task`, `arch`, `milestone`, `wave`,
  `gen`, `setup`, `upgrade`).

## Approach

Follow the existing `[[ci.extra_gates]]` / `[[codegen]]` idiom already in the
codebase: a declarative array-of-tables in `meta.toml`, executed **in-process**
by `meta` at lifecycle points. Commands are exec'd directly (tokenized via
`process::split_args`, no shell), exactly like `extra_gates` and `codegen`.

Alternative considered and rejected: a `.meta/hooks/<event>` executable-script
convention (à la git hooks). Rejected because the request is explicitly for
`meta.toml` configuration, and a parallel script directory would fragment where
a repo's behavior is declared.

## Schema — `[[hooks]]`

New array-of-tables in `meta.toml`:

```toml
[[hooks]]
event    = "pre_tag"                 # required; one of the known events
command  = "cargo test --workspace"  # required; exec'd directly (no shell)
name     = "tests"                   # optional label for output (default: command)
cwd      = "sdk"                     # optional run dir, relative to repo root
when_dir = "sdk"                     # optional; skip this hook unless sdk/ exists
```

Multiple entries may share the same `event`; they run in declaration order.

`HookEntry` (in `src/config/schema.rs`), mirroring `ExtraGate`:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookEntry {
    pub event: String,
    pub command: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub when_dir: Option<String>,
}
```

`MetaFile` gains `#[serde(default)] pub hooks: Vec<HookEntry>`. Hooks are always
user-supplied, so they are passed straight through to `EffectiveConfig` with no
profile-default merge (`hooks: file.hooks.clone()`).

## Supported events (curated)

| Command | Pre event   | Post event   |
|---------|-------------|--------------|
| `tag`   | `pre_tag`   | `post_tag`   |
| `init`  | `pre_init`  | `post_init`  |
| `build` | `pre_build` | `post_build` |
| `check` | `pre_check` | `post_check` |
| `ci`    | `pre_ci`    | `post_ci`    |

An `event` outside this set is a **hard config error** at load time, listing the
valid events. This is consistent with the schema's `deny_unknown_fields`
strictness (the string value can't be caught by serde, so we validate it
explicitly in `config::load`).

## Execution semantics

- **No shell.** Commands are tokenized and exec'd directly, like `extra_gates`
  and `codegen`. `&&`, pipes, `$VAR` expansion, and globs in the command string
  do **not** work. To run several steps, add several `[[hooks]]` entries for the
  same event (they run in order). To use env vars, read them inside the script
  the hook invokes — they are set in the child's environment, not expanded in
  the command string.
- **Order.** Declaration order within an event.
- **`when_dir`.** If set and the directory is absent (relative to repo root),
  the hook is skipped with a note.
- **`cwd`.** The hook runs in `root.join(cwd)`; default is the repo root.
- **Failure policy (fixed):**
  - `pre_*` → **abort.** The first non-zero hook stops the command; the
    command's own action never runs; `meta` exits non-zero.
  - `post_*` → **warn.** A non-zero hook prints a warning and execution
    continues; the command still reports success. The primary side effect
    (e.g. the tag push) is **not** rolled back.
- **No PATH auto-skip.** Unlike `extra_gates`/`codegen` (which SKIP when the
  program is not on PATH), hooks do **not** skip a missing program: it yields a
  normal non-zero exit. For `pre_*` hooks this is intentional fail-closed
  behavior — a typo'd guard aborts the release rather than silently "passing".
- **`--dry-run`.** For `tag` and `init`, hooks are **listed, not executed**.

## Environment variables

Every hook receives:

| Variable       | Value                                             |
|----------------|---------------------------------------------------|
| `META_ROOT`    | Absolute path to the repo root                    |
| `META_EVENT`   | The event name, e.g. `pre_tag`                    |
| `META_PROFILE` | The active profile (`rust`, `typescript`, …)      |

Tag hooks (`pre_tag`, `post_tag`) additionally receive:

| Variable            | Value                                        |
|---------------------|----------------------------------------------|
| `META_VERSION`      | The new/target version, e.g. `0.4.0`         |
| `META_PREV_VERSION` | The current version being bumped from, `0.3.0` |
| `META_TAG`          | The full tag name, e.g. `v0.4.0`             |

These are available to the child process and anything it spawns.

## Where hooks fire

- **`tag`** — `pre_tag` runs after all guardrails pass but **before** the version
  rewrite and commit, so a failing `pre_tag` leaves the tree untouched.
  `post_tag` runs after the successful `git push` of the commit and tag. Tag env
  vars (`META_VERSION`/`META_PREV_VERSION`/`META_TAG`) are known before the
  rewrite, so both phases get them.
- **`build`** — `pre_build` at the very start of the command, **before** code
  generators and the compile; `post_build` after the build completes.
- **`check`** — `pre_check` before the standards run, `post_check` after.
- **`ci`** — `pre_ci` at the start of the run, `post_ci` at the end.
- **`init`** — ⚠️ **bootstrapping semantics.** Init hooks are sourced from the
  `meta.toml` **already present on disk** at the time `meta init` runs, loaded
  via `config::load`. Rationale: init's in-memory config is built from the
  freshly *rendered* template (`config::load_from_str(&root, &meta_toml)`), which
  never contains user hooks. Consequences:
  - A first-ever `meta init` in an unconfigured repo has no prior `meta.toml`, so
    `pre_init`/`post_init` **no-op**.
  - In a repo whose `meta.toml` already defines init hooks (a re-run / refresh),
    `pre_init` runs before scaffolding and `post_init` after writing files.

  `pre_init` is guarded on the on-disk `meta.toml` existing; if it does not, the
  pre phase is skipped entirely.

## Code shape

New module `src/hooks.rs` — one focused, testable unit:

```rust
pub enum Phase { Pre, Post }

/// Run every hook registered for `event`, in declaration order, with
/// `META_*` env vars set. Pre: returns Err on the first failure (caller aborts).
/// Post: never errors — a failing hook warns and execution continues.
pub fn run(
    cfg: &EffectiveConfig,
    event: &str,
    phase: Phase,
    extra_env: &[(&str, String)],
) -> anyhow::Result<()>;
```

Supporting changes:

- `src/process.rs`: add `run_inherited_env(cmd, cwd, env: &[(&str, String)]) -> i32`
  — the existing `run_inherited` plus `.envs(env)` on the `Command`.
- `src/config/schema.rs`: `HookEntry`; `MetaFile.hooks`.
- `src/config/defaults.rs`: `hooks` field on `EffectiveConfig` (pass-through), and
  event validation surfaced through `config::load`.
- Call sites: `commands/tag.rs`, `commands/init.rs`, `commands/build.rs`,
  `commands/check.rs`, `commands/ci.rs`.
- Optional (discoverability): a commented `[[hooks]]` example in
  `scaffold::render_meta_toml`, and a hooks section in `docs/releasing.md`
  (or a new `docs/hooks.md`).

### Module boundaries

- `hooks::run` depends only on `EffectiveConfig` (for the hook list, root, and
  profile), `process` (to exec), and `output` (to print/warn). It knows nothing
  about individual commands.
- Each command owns *when* it calls `hooks::run` and *what* env it passes; the
  hooks module owns *how* hooks are selected, gated, exec'd, and how failures are
  handled per phase.

## Testing

- **Pure logic (unit):**
  - Selecting the hooks that match a given `event`, in order.
  - `META_*` env assembly (common + tag-specific).
  - `when_dir` gating (present vs absent).
  - Unknown-`event` validation error, with a helpful message.
- **Execution (integration, tempdir):**
  - `pre` phase: a failing hook returns Err and stops before subsequent hooks.
  - `post` phase: a failing hook warns and the run still returns Ok, and later
    hooks still run.
  - env vars are visible to the child (hook writes an env var to a file; test
    asserts contents).
  Use portable commands for cross-platform CI safety.

## Interaction notes

- The current release-flow mismatch (`meta tag` pushing the tag vs `release.yml`
  wanting to own tag creation) is **out of scope** here and tracked separately.
  Hooks do not change how the tag is created or pushed.
- The `Cargo.lock`-sync fix to `meta tag` (separate PR) is independent; both PRs
  touch `tag.rs` but in different regions and merge cleanly.
