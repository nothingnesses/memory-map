# Contributing

## Development Environment

This project uses Nix and nix-direnv to provide the Rust, frontend, database,
and object-storage tooling used during development.

```sh
cp .env.example .env
direnv allow
```

If `devenv/flake.nix` or `devenv/flake.lock` changes and a tool is missing from
your shell, rebuild the cached direnv environment:

```sh
rm -f .direnv/flake-profile* .direnv/nix-direnv.*
direnv allow
```

## Commands

Run project tasks through `just`:

```sh
just fmt             # Format Rust, Nix, Markdown, YAML, and TOML
just check           # Run cargo check
just clippy          # Run Clippy with warnings as errors
just deny            # Check dependency licenses and advisories
just doc             # Build docs and run ASCII/link checks
just test            # Run tests
just frontend-build  # Build the Trunk frontend
just verify          # Run the full verification suite
```

## Documentation And Text

Source and Markdown documentation are kept ASCII-only. Use plain ASCII
punctuation and tree diagrams so `just doc` can enforce this consistently.

## Pull Requests

Before opening a pull request, run:

```sh
just verify
```

Include tests for behavior changes. Avoid public API or user-visible behavior
changes unless the issue or pull request explicitly calls for them.
