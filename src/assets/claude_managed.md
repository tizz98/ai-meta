## Commands (via the `./meta` CLI)

This project is managed by [ai-meta](https://github.com/tizz98/ai-meta). The
`./meta` shim downloads the pinned binary (`.meta/version`) — no toolchain needed
just to run the standards checks.

- Build: `./meta build`{{#if has_build}}  (`{{build}}`){{/if}}
- Test: `./meta test`{{#if has_test}}  (`{{test}}`){{/if}}
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
- Keep new dependencies justified ({{deps_doc}}) or on the allowlist.
