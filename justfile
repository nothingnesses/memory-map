set dotenv-load
set shell := ["bash", "-c"]
set tempdir := "/tmp"

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
	{{ direnv_prefix }} bash -c 'cd backend; {{ log_prefix }} bacon run -- --bin backend {{ release_flag }}'

# Create local frontend runtime config when missing.
frontend-config:
	@if [ ! -f frontend/public/config.json ]; then \
		cp frontend/config.example.json frontend/public/config.json; \
		echo "Created frontend/public/config.json from frontend/config.example.json"; \
	fi

# Require an explicit frontend runtime config for builds that copy /config.json.
require-frontend-config:
	@if [ ! -f frontend/public/config.json ]; then \
		echo "ERROR: frontend/public/config.json is required for frontend-build." >&2; \
		echo "Run 'just frontend-config' for local development or provide a deployment-specific config file." >&2; \
		exit 1; \
	fi

# Start frontend server. https://github.com/trunk-rs/trunk/issues/732#issuecomment-2391810077
frontend: frontend-config
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
	for path in README.md CONTRIBUTING.md AGENTS.md docs frontend/README.md backend/src frontend/src shared/src; do
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

	{{ direnv_prefix }} lychee --offline --no-progress README.md docs/deployment.md frontend/README.md

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
frontend-build: require-frontend-config
	{{ direnv_prefix }} bash -c 'cd frontend && pnpm install --frozen-lockfile --prefer-offline && env -u NO_COLOR trunk build {{ release_flag }} --skip-version-check'

# Typecheck the Playwright E2E TypeScript support code and specs.
frontend-e2e-typecheck:
	{{ direnv_prefix }} bash -c 'cd frontend && pnpm install --frozen-lockfile --prefer-offline && pnpm run e2e:typecheck'

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
	source scripts/service-graph.sh

	log_file="${PROCESS_COMPOSE_LOG:-process-compose.log}"
	port="${PROCESS_COMPOSE_PORT:-8080}"
	process_compose_started=false

	cleanup() {
		if [[ "$process_compose_started" == "true" ]]; then
			memory_map_process_compose --port "$port" down || true
		fi
	}
	trap cleanup EXIT

	memory_map_start_process_compose --port "$port" --log-file "$log_file" --detached -t=false --logs-truncate
	process_compose_started=true
	memory_map_process_compose --port "$port" project is-ready --wait
	BACKEND_TEST_REQUIRE_SERVICE=true just storage-test

# Run backend API/auth integration tests against configured local services.
[positional-arguments]
backend-integration-test *args:
	#!/usr/bin/env bash
	set -euo pipefail
	if [ "$#" -eq 0 ]; then
		set -- --ignored --nocapture --test-threads=1
	fi
	source scripts/e2e-env.sh
	BACKEND_TEST_REQUIRE_SERVICE=true {{ direnv_prefix }} cargo test -p backend --test api_auth -- "$@"

# Run backend API/auth integration tests against the headless local service graph.
backend-integration:
	#!/usr/bin/env bash
	set -euo pipefail

	source scripts/e2e-env.sh
	source scripts/service-graph.sh
	log_dir="${BACKEND_INTEGRATION_LOG_DIR:-backend-integration-logs}"
	log_file="${BACKEND_INTEGRATION_PROCESS_COMPOSE_LOG:-$log_dir/process-compose.log}"
	port="${BACKEND_INTEGRATION_PROCESS_COMPOSE_PORT:-$PROCESS_COMPOSE_PORT}"
	mkdir -p "$log_dir"
	process_compose_started=false

	cleanup() {
		if [[ "$process_compose_started" == "true" ]]; then
			memory_map_process_compose --port "$port" down >> "$log_dir/process-compose-down.log" 2>&1 || true
		fi
	}
	trap cleanup EXIT

	memory_map_require_port_free "$E2E_PG_PORT" "PostgreSQL"
	memory_map_require_port_free "$E2E_STORAGE_API_PORT" "RustFS API"
	memory_map_require_port_free "$E2E_STORAGE_CONSOLE_PORT" "RustFS console"
	memory_map_require_port_free "$port" "process-compose"

	memory_map_start_process_compose --port "$port" --log-file "$log_file" --detached -t=false --logs-truncate
	process_compose_started=true
	memory_map_process_compose --port "$port" project is-ready --wait
	just backend-integration-test

# Run Playwright E2E tests against the headless local service graph.
e2e: frontend-config frontend-e2e-typecheck
	#!/usr/bin/env bash
	set -euo pipefail

	source scripts/e2e-env.sh
	source scripts/service-graph.sh
	mkdir -p "$E2E_LOG_DIR"
	process_compose_started=false

	wait_for_backend() {
		local response
		local query='{"query":"query ConfigQuery { config { enableRegistration } }"}'

		for _ in $(seq 1 120); do
			if response=$(curl --fail --silent --show-error --max-time 2 \
				-H 'Content-Type: application/json' \
				--data "$query" \
				"$E2E_BACKEND_URL/" 2>/dev/null) \
				&& [[ "$response" == *'"enableRegistration"'* ]]; then
				return 0
			fi
			sleep 1
		done

		echo "ERROR: backend did not become ready; see $E2E_LOG_DIR/backend.log." >&2
		tail -n 80 "$E2E_LOG_DIR/backend.log" >&2 || true
		exit 1
	}

	wait_for_frontend() {
		for _ in $(seq 1 120); do
			if curl --fail --silent --show-error --max-time 2 "$E2E_FRONTEND_URL/" >/dev/null 2>&1; then
				return 0
			fi
			sleep 1
		done

		echo "ERROR: frontend did not become ready; see $E2E_LOG_DIR/frontend.log." >&2
		tail -n 80 "$E2E_LOG_DIR/frontend.log" >&2 || true
		exit 1
	}

	cleanup() {
		local status=$?

		trap - EXIT INT TERM
		memory_map_stop_pid "${frontend_pid:-}"
		memory_map_stop_pid "${backend_pid:-}"
		if [[ "$process_compose_started" == "true" ]]; then
			memory_map_process_compose --port "$PROCESS_COMPOSE_PORT" down >> "$E2E_LOG_DIR/process-compose-down.log" 2>&1 || true
		fi
		exit "$status"
	}
	trap cleanup EXIT INT TERM

	memory_map_require_port_free "$E2E_PG_PORT" "PostgreSQL"
	memory_map_require_port_free "$E2E_STORAGE_API_PORT" "RustFS API"
	memory_map_require_port_free "$E2E_STORAGE_CONSOLE_PORT" "RustFS console"
	memory_map_require_port_free "$E2E_BACKEND_PORT" "backend"
	memory_map_require_port_free "$E2E_FRONTEND_PORT" "frontend"
	memory_map_require_port_free "$PROCESS_COMPOSE_PORT" "process-compose"

	{{ direnv_prefix }} cargo build --bin backend
	{{ direnv_prefix }} bash -c 'cd frontend && pnpm install --frozen-lockfile --prefer-offline && env -u NO_COLOR trunk build --skip-version-check --offline'

	memory_map_start_process_compose --port "$PROCESS_COMPOSE_PORT" --log-file "$PROCESS_COMPOSE_LOG" --detached -t=false --logs-truncate
	process_compose_started=true
	memory_map_process_compose --port "$PROCESS_COMPOSE_PORT" project is-ready --wait

	{{ direnv_prefix }} bash -c 'cd backend && exec cargo run --bin backend' > "$E2E_LOG_DIR/backend.log" 2>&1 &
	backend_pid=$!
	wait_for_backend

	{{ direnv_prefix }} bash -c 'cd frontend && exec env -u NO_COLOR trunk serve --address "$E2E_FRONTEND_HOST" --port "$E2E_FRONTEND_PORT" --no-autoreload --skip-version-check --offline' > "$E2E_LOG_DIR/frontend.log" 2>&1 &
	frontend_pid=$!
	wait_for_frontend

	{{ direnv_prefix }} bash -c 'cd frontend && exec pnpm exec playwright test'

# Remove build artifacts.
clean:
	{{ direnv_prefix }} cargo clean

# Check licenses and advisories with cargo-deny.
deny:
	#!/usr/bin/env bash
	set -euo pipefail

	if [[ "${CI:-}" == "true" && -z "${CARGO_HOME:-}" ]]; then
		export CARGO_HOME="${RUNNER_TEMP:-$PWD/.cargo-deny}/cargo-home"
	fi
	cargo_home="${CARGO_HOME:-$HOME/.cargo}"

	prepare_ci_advisory_db() {
		local advisory_root="$cargo_home/advisory-dbs"
		# cargo-deny 0.18+ documents this stable directory for the default RustSec DB URL.
		local advisory_db="$advisory_root/advisory-db-3157b0e258782691"

		mkdir -p "$advisory_root"
		if [[ ! -d "$advisory_db/.git" ]]; then
			rm -rf "$advisory_db"
			git clone --depth 1 https://github.com/rustsec/advisory-db "$advisory_db"
		fi

		git -C "$advisory_db" fetch --depth 1 origin HEAD
		git -C "$advisory_db" reset --hard FETCH_HEAD
	}

	run_deny() {
		if [[ "${CI:-}" == "true" ]]; then
			prepare_ci_advisory_db
			{{ direnv_prefix }} cargo deny check --disable-fetch
		else
			{{ direnv_prefix }} cargo deny check
		fi
	}

	if ! run_deny; then
		if [[ "${CI:-}" != "true" ]]; then
			exit 1
		fi

		rm -rf "$cargo_home/advisory-dbs"
		run_deny
	fi

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
		backend-integration|backend-integration-test|build|check|clippy|deny|doc|e2e|fmt|frontend-build|frontend-e2e-typecheck|storage-ci|storage-test|test|verify) ;;
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
	just frontend-e2e-typecheck
	just frontend-config
	just frontend-build
