#!/usr/bin/env bash

memory_map_run_in_dev_env() {
	if [[ -n "${SKIP_DIRENV:-}" ]]; then
		"$@"
	else
		direnv exec . "$@"
	fi
}

memory_map_start_process_compose() {
	if [[ "${PROCESS_COMPOSE_BIN:-default}" == "default" ]]; then
		memory_map_run_in_dev_env nix run ./devenv -- "$@"
	else
		memory_map_run_in_dev_env "$PROCESS_COMPOSE_BIN" "$@"
	fi
}

memory_map_process_compose() {
	memory_map_run_in_dev_env process-compose "$@"
}

memory_map_require_port_free() {
	local port="$1"
	local name="$2"

	if (echo >"/dev/tcp/127.0.0.1/$port") >/dev/null 2>&1; then
		echo "ERROR: $name port $port is already in use." >&2
		exit 1
	fi
}

memory_map_stop_pid() {
	local pid="${1:-}"

	if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
		kill "$pid" 2>/dev/null || true
		wait "$pid" 2>/dev/null || true
	fi
}
