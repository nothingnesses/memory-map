set dotenv-load
set shell := ["bash", "-c"]

# Load the Nix development environment via direnv for local recipes. CI invokes
# recipes via `nix develop --command just`, so SKIP_DIRENV=1 bypasses the prefix.
skip_direnv := env_var_or_default("SKIP_DIRENV", "")
direnv_prefix := if skip_direnv != "" { "" } else { "direnv exec . " }

# Build mode configuration.
mode := env_var_or_default("BUILD_MODE", "debug")
release_flag := if mode == "release" { "--release" } else { "" }
log_prefix := if mode == "debug" { "RUST_LOG=debug" } else { "" }

# Display list of commands.
default:
	@just --list

# Approve the direnv environment after reviewing `.envrc` and Nix flake changes.
allow-env:
	direnv allow

# Start PostgreSQL and RustFS servers.
servers:
	{{ direnv_prefix }} nix run ./devenv

# Remove local service state created by current and legacy service recipes.
clean-service-state:
	rm -rf data/postgres data/rustfs data/pg1 data/minio1

# Start backend server.
backend:
	{{ direnv_prefix }} bash -c 'cd backend; {{ log_prefix }} bacon run -- {{ release_flag }}'

# Start frontend server. https://github.com/trunk-rs/trunk/issues/732#issuecomment-2391810077
frontend:
	{{ direnv_prefix }} bash -c 'cd frontend; ping -c 1 8.8.8.8 && pnpm i --prefer-offline; env -u NO_COLOR trunk serve {{ release_flag }} --skip-version-check --offline --open'

# Regenerate "frontend/graphql/schema.json".
regenerate-schema:
	{{ direnv_prefix }} graphql-client introspect-schema http://localhost:8000 > frontend/graphql/schema.json

# Format all files through the Nix formatter.
fmt:
	{{ direnv_prefix }} bash -c 'cd devenv && nix fmt'

# Check without building.
[positional-arguments]
check *args:
	#!/usr/bin/env bash
	set -euo pipefail
	if [ "$#" -eq 0 ]; then
		set -- --workspace --all-targets --all-features
	fi
	{{ direnv_prefix }} cargo check "$@"

# Run clippy with warnings treated as errors.
[positional-arguments]
clippy *args:
	#!/usr/bin/env bash
	set -euo pipefail
	if [ "$#" -eq 0 ]; then
		set -- --workspace --all-targets --all-features
	fi
	{{ direnv_prefix }} cargo clippy "$@" -- -D warnings

# Build documentation with warnings treated as errors and enforce ASCII docs.
[positional-arguments]
doc *args:
	#!/usr/bin/env bash
	set -euo pipefail

	ascii_roots=()
	for path in README.md CONTRIBUTING.md AGENTS.md frontend/README.md backend/src frontend/src shared/src; do
		if [[ -e "$path" ]]; then
			ascii_roots+=("$path")
		fi
	done

	matches=$({{ direnv_prefix }} rg -nP '[^[:ascii:]]' "${ascii_roots[@]}" -g '*.rs' -g '*.md' || true)
	if [[ -n "$matches" ]]; then
		echo "ERROR: Non-ASCII characters found in source or documentation files. Use ASCII equivalents." >&2
		echo "" >&2
		echo "Offending lines:" >&2
		echo "$matches" >&2
		exit 1
	fi

	{{ direnv_prefix }} lychee --offline --no-progress README.md frontend/README.md

	if [ "$#" -eq 0 ]; then
		set -- --workspace --all-features --no-deps
	fi
	{{ direnv_prefix }} env RUSTDOCFLAGS="-D warnings" cargo doc "$@"

# Build the Rust workspace.
[positional-arguments]
build *args:
	#!/usr/bin/env bash
	set -euo pipefail
	if [ "$#" -eq 0 ]; then
		set -- --workspace --all-targets --all-features
	fi
	{{ direnv_prefix }} cargo build "$@"

# Build the frontend application through Trunk.
frontend-build:
	{{ direnv_prefix }} bash -c 'cd frontend && pnpm install --frozen-lockfile --prefer-offline && env -u NO_COLOR trunk build {{ release_flag }} --skip-version-check'

# Run any cargo subcommand except test; use `just test` for tests.
[positional-arguments]
cargo *args:
	#!/usr/bin/env bash
	set -euo pipefail
	if [ "$#" -eq 0 ]; then
		echo "ERROR: cargo subcommand required." >&2
		exit 2
	fi
	if [ "$1" = "test" ]; then
		echo "ERROR: Use 'just test' instead of 'just cargo test'." >&2
		exit 1
	fi
	{{ direnv_prefix }} cargo "$@"

# Run tests.
[positional-arguments]
test *args:
	#!/usr/bin/env bash
	set -euo pipefail
	if [ "$#" -eq 0 ]; then
		set -- --workspace --all-features
	fi
	{{ direnv_prefix }} cargo test "$@"

# Run storage integration tests against a configured S3-compatible service.
[positional-arguments]
storage-test *args:
	#!/usr/bin/env bash
	set -euo pipefail
	if [ "$#" -eq 0 ]; then
		set -- --ignored --nocapture
	fi
	{{ direnv_prefix }} cargo test -p backend --test storage -- "$@"

# Run storage integration tests against the headless local service graph.
storage-ci:
	#!/usr/bin/env bash
	set -euo pipefail

	log_file="${PROCESS_COMPOSE_LOG:-process-compose.log}"
	port="${PROCESS_COMPOSE_PORT:-8080}"

	cleanup() {
		{{ direnv_prefix }} nix run ./devenv -- --port "$port" down || true
	}
	trap cleanup EXIT

	{{ direnv_prefix }} nix run ./devenv -- --port "$port" --log-file "$log_file" --detached -t=false --logs-truncate
	{{ direnv_prefix }} nix run ./devenv -- --port "$port" project is-ready --wait
	STORAGE_TEST_REQUIRE_SERVICE=true just storage-test

# Remove build artifacts.
clean:
	{{ direnv_prefix }} cargo clean

# Check licenses and advisories with cargo-deny.
deny:
	{{ direnv_prefix }} cargo deny check

# Run an allowed just recipe and filter its output with a caller-provided rg regex.
[positional-arguments]
filtered recipe filter *args:
	#!/usr/bin/env bash
	set -euo pipefail

	recipe="$1"
	filter="$2"
	shift 2

	if [ -z "$filter" ]; then
		echo "ERROR: filtered requires a non-empty rg regex." >&2
		exit 2
	fi

	case "$recipe" in
		build|check|clippy|deny|doc|fmt|frontend-build|storage-ci|storage-test|test|verify) ;;
		*)
			echo "ERROR: unsupported filtered recipe: $recipe" >&2
			exit 2
			;;
	esac

	for arg in "$@"; do
		case "$arg" in
			*$'\n'*|*$'\r'*|*[\;\&\|\\\<\>\`\$\'\"\(\)\{\}]*)
				echo "ERROR: unsafe filtered recipe argument: $arg" >&2
				exit 2
				;;
		esac
	done

	output=$(mktemp -t just-filtered.XXXXXX)
	trap 'rm -f "$output"' EXIT

	set +e
	just --one "$recipe" "$@" > "$output" 2>&1
	recipe_status=$?
	set -e

	rg_status=0
	rg -n -m 300 -- "$filter" "$output" || rg_status=$?
	if [ "$rg_status" -eq 2 ]; then
		exit 2
	fi

	if [ "$rg_status" -ne 0 ] && [ "$recipe_status" -ne 0 ]; then
		echo "=== no filter matches; last 80 lines ===" >&2
		tail -n 80 "$output" >&2
	fi

	exit "$recipe_status"

# Scan for hardcoded values.
scan-hardcoded:
	./scripts/scan_hardcoded.sh

# Verify: fmt, check, clippy, deny, doc, test, frontend build.
verify:
	just fmt
	just check
	just clippy
	just deny
	just doc
	just test
	just frontend-build
