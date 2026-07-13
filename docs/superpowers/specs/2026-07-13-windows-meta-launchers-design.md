# Native Windows launchers for `meta` — design

Date: 2026-07-13
Status: approved (pending implementation)

## Problem

On Windows, the only per-repo launcher `meta init`/`upgrade` generates is `meta`
— a **bash** script. It resolves the pinned binary only under Git Bash / MSYS2 /
Cygwin (its `case` matches `MINGW*`/`MSYS*`/`CYGWIN*`). From native **PowerShell**
or **cmd.exe** there is no launcher at all, so `./meta check` does nothing useful.
The global installer `install.sh` is likewise bash-only.

Goal: `./meta <cmd>` is **transparent** — the same command a user reads in the
docs, CLAUDE.md, CI, and skills works unchanged in bash, PowerShell, and cmd.exe,
on a stock Windows machine, with **no** execution-policy setup.

## Non-goals

- ARM64 Windows binaries (none are published yet — error clearly instead).
- Auto-editing the user's `PATH` or execution policy (the installer *prints*
  guidance, mirroring `install.sh`).
- Changing the bash `meta` shim or any non-Windows behavior.
- Running the generated GitHub CI on Windows runners (workflows stay on Ubuntu).

## Resolution model (the transparency guarantee)

After `init`/`upgrade`, the repo root carries `meta` (bash) and `meta.cmd`
(Windows dispatcher); the PowerShell logic lives at `.meta/shim.ps1`. Each shell
auto-selects its own launcher and ignores the others:

| Shell | User types | Resolves to | Execution-policy proof? |
|---|---|---|---|
| bash / Git Bash | `./meta check` | `meta` (bash, shebang) | n/a |
| PowerShell 7 & 5.1 | `./meta check` | `meta.cmd` (no root `.ps1` to intercept) → `powershell -ExecutionPolicy Bypass -File .meta\shim.ps1` → binary | yes |
| cmd.exe | `.\meta check` | `meta.cmd` → same | yes |

The same `./meta <cmd>` string works in **bash and PowerShell** (both accept a
forward slash). cmd.exe users type `.\meta`.

**Why the PowerShell logic is hidden in `.meta/` and not a root `meta.ps1`:**
PowerShell ranks a `.ps1` in the current directory **above** a PATHEXT `.cmd`
(verified on 7.5 and 5.1). A directly-invoked `.ps1` is subject to execution
policy, so on a Restricted machine (Windows Server default / locked-down GPO)
`./meta` would fail with `"... cannot be loaded because running scripts is
disabled on this system."` By keeping no `.ps1` at the repo root, PowerShell's
`./meta` falls through to `meta.cmd`, which launches PowerShell with
`-ExecutionPolicy Bypass` for the child process — immune to the machine policy.
(See Appendix for the empirical probe.)

## Components

### 1. `src/assets/shim.cmd` → generated `meta.cmd` (repo root)

Windows dispatcher. Prefers PowerShell 7 (`pwsh`, faster) and falls back to
Windows PowerShell 5.1 (`powershell`, always present). Always launches the child
with `-ExecutionPolicy Bypass`, forwards all args verbatim via `%*`, and
propagates the exit code.

```bat
@echo off
setlocal
set "PS=powershell"
where /q pwsh && set "PS=pwsh"
"%PS%" -NoProfile -ExecutionPolicy Bypass -File "%~dp0.meta\shim.ps1" %*
exit /b %errorlevel%
```

Notes:
- `%~dp0` is the dispatcher's own directory (trailing `\`), so `.meta\shim.ps1`
  resolves regardless of the caller's cwd (e.g. invoking from a subdir).
- Single PowerShell invocation. A `where /q pwsh && (A) || (B)` idiom is **wrong**
  here: `meta check` legitimately returns nonzero on lint failure, which would
  trigger the `||` branch and run the command twice. The `set "PS=..."` form runs
  it exactly once.
- Linear batch (no labels/`goto`), so LF line endings are safe; verification
  confirms it runs. If a stray issue appears, pin CRLF via `.gitattributes`.

### 2. `src/assets/shim.ps1` → generated `.meta/shim.ps1`

PowerShell mirror of `shim.sh`. 5.1-compatible (no PS7-only syntax).

Behavior, in order:
1. `$ver` = trimmed contents of `Join-Path $PSScriptRoot 'version'` (i.e.
   `.meta/version`, since the shim itself lives in `.meta/`). Empty/missing →
   `Write-Error "meta: missing .meta/version (run 'meta init')"; exit 1`
   (same text as bash).
2. Map host arch (`$env:PROCESSOR_ARCHITECTURE`): `AMD64`/`x86` →
   `tgt=x86_64-pc-windows-msvc`, `ext=.exe`. Anything else (e.g. `ARM64`) →
   `Write-Error "meta: unsupported Windows arch <arch> (no published build)"; exit 1`.
3. `$cache` = `$env:AI_META_CACHE` else `Join-Path $HOME '.cache/ai-meta'`, then
   `Join-Path $cache $ver`. Same path shape as the bash shim, so Git Bash and
   PowerShell **share** the download cache (`$HOME` = `%USERPROFILE%` in both).
4. `$asset = "ai-meta-$tgt$ext"`, `$bin = Join-Path $cache $asset`.
5. If `$bin` is absent: create `$cache`; set
   `[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12`
   (PS 5.1 default may exclude TLS 1.2) and `$ProgressPreference='SilentlyContinue'`;
   `Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset" -OutFile "$bin.tmp"`
   where `$base = "https://github.com/tizz98/ai-meta/releases/download/v$ver"`.
   Download failure → error + `exit 1`.
6. Best-effort checksum (mirror bash — skip if none published): download
   `"$asset.sha256"`, `$want = first whitespace token, lowercased`;
   `$have = (Get-FileHash -Algorithm SHA256 "$bin.tmp").Hash.ToLower()`; on
   mismatch delete the temp file, error, `exit 1`.
7. `Move-Item -Force "$bin.tmp" $bin`.
8. `& $bin @args; exit $LASTEXITCODE` — cwd is inherited from the caller (repo
   root), matching bash `exec "$bin" "$@"`.

Arg fidelity: `@args` splatting forwards normal `subcommand --flags "quoted"`
usage correctly. Exotic embedded-quote arguments can degrade across the
cmd → powershell → exe hop; acceptable for the meta CLI surface, noted as a test
target.

### 3. `src/scaffold.rs`

- `const SHIM_CMD: &str = include_str!("assets/shim.cmd");`
- `const SHIM_PS1: &str = include_str!("assets/shim.ps1");`
- In `generated_artifacts()`, after the existing `meta` artifact, push two more,
  both `Ownership::Generated`, `executable: false`:
  - `path: "meta.cmd"`, `content: SHIM_CMD`
  - `path: ".meta/shim.ps1"`, `content: SHIM_PS1`
- These are generated on **all** platforms so a shared repo carries every
  launcher regardless of which OS ran `init`. On Linux/macOS the two files are
  inert data. `write_file` already creates the `.meta/` parent; `.meta/shim.ps1`
  is committed (only `/.meta/state/` and `/.meta/bin/` are git-ignored).
- Tests: extend `generates_expected_artifacts_for_rust` (or add a test) asserting
  both paths are present, `Generated`, non-executable, and content sanity
  (`meta.cmd` contains `shim.ps1`; `shim.ps1` contains `.meta/version`'s read and
  `x86_64-pc-windows-msvc`). Existing `meta` (bash) assertions stay.

No other Rust changes: `init`, `upgrade`, and `sync` all iterate
`generated_artifacts()`, so the two new files flow through diff/apply
automatically.

### 4. `install.ps1` (repo root, hand-maintained — NOT scaffolded)

Native mirror of `install.sh`. Served via
`irm https://raw.githubusercontent.com/tizz98/ai-meta/main/install.ps1 | iex`.

- Env tunables: `AI_META_VERSION` (pin; tolerate leading `v`), `AI_META_BIN_DIR`
  (default `Join-Path $env:LOCALAPPDATA 'ai-meta\bin'`), `AI_META_REPO`
  (default `tizz98/ai-meta`).
- `$ErrorActionPreference='Stop'`, TLS 1.2, `$ProgressPreference='SilentlyContinue'`.
- Arch → `x86_64-pc-windows-msvc.exe`; non-AMD64/x86 → throw "unsupported".
- Base URL: pinned `.../releases/download/v$version` else `.../releases/latest/download`.
- Download `ai-meta-<tgt>.exe` to a temp dir; best-effort `Get-FileHash` sha256
  verify against `.sha256`; on mismatch throw, otherwise move to
  `$binDir\meta.exe` (creating `$binDir`).
- If `$binDir` is not already on `$env:PATH`, **print** (not run) the User-scope
  PATH one-liner and a "restart your shell" note. Never modify persistent env.
- Final line: `done. Run 'meta init' in a repo to get started.`

### 5. `README.md`

Update the Windows paragraph (currently "run `install.sh` and the `./meta` shim
from Git Bash / MSYS2 / Cygwin"): add the native path —
`irm https://raw.githubusercontent.com/tizz98/ai-meta/main/install.ps1 | iex`,
and note `./meta <cmd>` works in PowerShell and `.\meta <cmd>` in cmd.exe, under
any execution policy, with Git Bash still supported.

## Edge cases

- Missing `.meta/version` → identical error text to bash.
- ARM64 Windows → clear "no published build" error (both shim and installer).
- Checksum mismatch → temp deleted, nonzero exit.
- No checksum published → warn + proceed (mirror bash).
- Restricted / AllSigned execution policy → unaffected: PowerShell reaches the
  binary through `meta.cmd`'s `-ExecutionPolicy Bypass` child.
- Repo downloaded as a zip (Mark-of-the-Web) → still fine: the child launched by
  `meta.cmd` bypasses policy; nothing runs a MOTW-tagged `.ps1` directly.

## Testing / verification

Rust:
- `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- New/updated scaffold tests as above.

Dogfood on this Windows box (v0.4.0 shipped the Windows asset, so the download
path is exercised for real):
- `cargo run -- upgrade` → writes `meta.cmd` + `.meta/shim.ps1` into this repo.
- PowerShell: `./meta check` → falls to `meta.cmd` → `.meta\shim.ps1` → downloads
  `ai-meta-x86_64-pc-windows-msvc.exe` for the pinned version → runs the check.
- cmd.exe: `.\meta check` → same.
- Confirm PowerShell 5.1 also resolves `./meta` → `meta.cmd` in the no-`.ps1`
  layout (parity with 7.5).
- Exit-code propagation: a failing `check` returns nonzero through both hops and
  is not double-run.
- `install.ps1` in a scratch dir with `AI_META_BIN_DIR` set to a temp path →
  installs `meta.exe`, prints the PATH note.

## Files touched

- new: `src/assets/shim.cmd`, `src/assets/shim.ps1`, `install.ps1`,
  this spec.
- edit: `src/scaffold.rs`, `README.md`.
- regenerated by dogfood: `meta.cmd`, `.meta/shim.ps1` (committed).

## Appendix — empirical probe (this box)

PowerShell 7.5.8; `PATHEXT` includes `.CMD` (not `.PS1`). In a dir holding
`meta` (extensionless), `meta.cmd`, and `meta.ps1`:

- PS 7 and PS 5.1: `./meta` → **`meta.ps1`** (ExternalScript outranks `.cmd`,
  extensionless last). Removing `meta.ps1`: `./meta` → **`meta.cmd`**.
- `meta.ps1` run directly under `-ExecutionPolicy RemoteSigned` (Win10/11 client
  default) with no Mark-of-the-Web (git clone / generated file) → **runs**.
- Same under `-ExecutionPolicy Restricted` → **blocked** (`PSSecurityException`,
  `UnauthorizedAccess`).
- Generated/local files carry **no** `Zone.Identifier` stream.
- cmd.exe: `.\meta` → `meta.cmd`.

These are why Option B (route PowerShell through `meta.cmd`, keep no root `.ps1`)
is policy-proof, and why the bash `meta` and the two Windows launchers coexist
without collision.
