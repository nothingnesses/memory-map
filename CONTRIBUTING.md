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
just verify-fast     # Fast checks; skips the service-backed suites
just verify          # Full suite, incl. service-backed integration and e2e
```

### Filtered Command Output

When a `just` recipe is expected to produce a large amount of output, use
`just filtered` instead of writing an ad-hoc shell pipeline. The first argument
is the `just` recipe to run, the second argument is a ripgrep regex used to
select output lines, and the remaining arguments are forwarded to the selected
recipe.

```sh
just filtered check '^(error|warning|[[:space:]]*-->)'
just filtered test '^(test .* \.\.\. FAILED|failures:|error)'
just filtered verify '^(Recipe|error|warning|failures:|FAILED|test result:)'
```

`just filtered` preserves the selected recipe's exit status, rejects unsupported
recipes and unsafe forwarded arguments, limits filtered matches so accidental
broad filters do not dump full logs, and prints the last captured lines when a
failing command has no filter matches.

## Documentation And Text

Source and Markdown documentation are kept ASCII-only. Use plain ASCII
punctuation and tree diagrams so `just doc` can enforce this consistently.

## Pull Requests

Before opening a pull request, run:

```sh
just verify
```

This runs the full suite, including the service-backed storage,
backend-integration, and e2e tests, which stand up the local Postgres + RustFS
service graph and take several minutes. For faster iteration while developing,
`just verify-fast` runs everything except those service-backed suites.

Include tests for behavior changes. Avoid public API or user-visible behavior
changes unless the issue or pull request explicitly calls for them.
