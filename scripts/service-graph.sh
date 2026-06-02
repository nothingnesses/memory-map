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

memory_map_wait_for_http() {
	local url="$1"
	local timeout_seconds="$2"
	local match_substring="${3:-}"
	local response

	for _ in $(seq 1 "$timeout_seconds"); do
		if response=$(curl --fail --silent --show-error --max-time 2 "$url" 2>/dev/null); then
			if [[ -z "$match_substring" || "$response" == *"$match_substring"* ]]; then
				return 0
			fi
		fi
		sleep 1
	done

	echo "ERROR: $url did not become ready within ${timeout_seconds}s." >&2
	return 1
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

memory_map_with_process_compose() {
	local port="$1"
	local log_path="$2"
	shift 2

	if [[ "${1:-}" != "--" ]]; then
		echo "ERROR: memory_map_with_process_compose requires -- before the command." >&2
		return 2
	fi
	shift
	if [[ "$#" -eq 0 ]]; then
		echo "ERROR: memory_map_with_process_compose requires a command." >&2
		return 2
	fi

	local log_dir
	log_dir="$(dirname "$log_path")"
	if [[ "$log_dir" != "." ]]; then
		mkdir -p "$log_dir"
	fi
	local down_log="$log_dir/process-compose-down.log"
	local process_compose_started=false

	memory_map_process_compose_cleanup() {
		if [[ "$process_compose_started" == "true" ]]; then
			memory_map_process_compose --port "$port" down >> "$down_log" 2>&1 || true
			process_compose_started=false
		fi
	}

	trap 'status=$?; memory_map_process_compose_cleanup; trap - EXIT INT TERM; exit "$status"' EXIT INT TERM

	memory_map_start_process_compose --port "$port" --log-file "$log_path" --detached -t=false --logs-truncate
	process_compose_started=true
	memory_map_process_compose --port "$port" project is-ready --wait

	set +e
	(set -e; "$@")
	local command_status=$?
	set -e

	memory_map_process_compose_cleanup
	trap - EXIT INT TERM
	return "$command_status"
}
