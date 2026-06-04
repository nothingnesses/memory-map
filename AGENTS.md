# AGENTS.md

This file contains project guidance for AI coding assistants.

## Commands

Run repository tasks through `just`. The `justfile` loads the Nix development
environment through direnv for local use. In CI, recipes run inside
`nix develop ./devenv/ --command just` with `SKIP_DIRENV=1`.

Common commands:

```sh
just fmt
just check
just clippy
just deny
just doc
just test
just frontend-build
just verify-fast
just verify
```

### Filtered Output

Use `just filtered` when a `just` recipe is expected to produce noisy output.
Prefer it over hand-written shell pipelines such as `2>&1 | rg ...` because it
preserves the selected recipe's exit status, rejects unsupported recipes and
unsafe forwarded arguments, caps filtered matches, and prints the last captured
lines when a failing command has no filter matches.

Examples:

```sh
just filtered check '^(error|warning|[[:space:]]*-->)'
just filtered test '^(test .* \.\.\. FAILED|failures:|error)'
just filtered verify '^(Recipe|error|warning|failures:|FAILED|test result:)'
```

The first argument is the recipe, the second argument is the `rg` regex, and any
remaining arguments are forwarded to the selected recipe. Continue using
targeted `sed` ranges, `git diff --stat`, `git diff --name-only`, or
command-specific quiet flags for non-`just` output. Avoid dumping full logs,
full diffs, or broad command output unless explicitly requested.

## Project Shape

- `backend/` contains the Axum, GraphQL, PostgreSQL, RustFS, and Casbin backend.
- `frontend/` contains the Leptos CSR application built by Trunk.
- `shared/` contains Rust code shared by the workspace crates.
- `devenv/` contains the Nix development environment and service definitions.
- `docs/` contains long-form documentation (e.g. `deployment.md`).
- `scripts/` contains shared shell helpers used by `justfile` recipes
  (e.g. `e2e-env.sh`, `service-graph.sh`).

## Editing Guidelines

- Keep source and Markdown documentation ASCII-only.
- Preserve the repository's hard-tab Rust/TOML formatting style.
- Prefer existing task recipes over ad-hoc shell commands.
- Keep public API and behavior stable unless the user explicitly approves a
  behavior change.
- When GraphQL schema changes are needed, start the backend and run
  `just regenerate-schema`.

## Verification

After changes, run the narrowest useful command first, then run `just verify`
before final handoff when feasible. `just verify` runs the full suite, including
the service-backed storage, backend-integration, and e2e tests that stand up the
local Postgres + RustFS service graph; use `just verify-fast` for a quick gate
when the service graph is unavailable or for fast iteration. If a command fails
because dependencies or network access are unavailable, report that directly.
