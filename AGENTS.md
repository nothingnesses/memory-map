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
just verify
```

Use `just filtered <recipe> <rg-filter> [args...]` for noisy verification output.

## Project Shape

- `backend/` contains the Axum, GraphQL, PostgreSQL, MinIO, and Casbin backend.
- `frontend/` contains the Leptos CSR application built by Trunk.
- `shared/` contains Rust code shared by the workspace crates.
- `devenv/` contains the Nix development environment and service definitions.

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
before final handoff when feasible. If a command fails because dependencies or
network access are unavailable, report that directly.
