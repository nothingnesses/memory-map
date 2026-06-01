#!/usr/bin/env bash

export E2E_FRONTEND_URL="${E2E_FRONTEND_URL:-http://127.0.0.1:3000}"
export E2E_BACKEND_URL="${E2E_BACKEND_URL:-http://127.0.0.1:8000}"

export MEMORY_MAP__FRONTEND__URL="$E2E_FRONTEND_URL"
export MEMORY_MAP__CORS__ALLOWED_ORIGINS="$E2E_FRONTEND_URL"
export MEMORY_MAP__SERVER__HOST="127.0.0.1"
export MEMORY_MAP__SERVER__PORT="8000"

export MEMORY_MAP__PG__DBNAME="db"
export MEMORY_MAP__PG__HOST="127.0.0.1"
export MEMORY_MAP__PG__PORT="5432"

export MEMORY_MAP__STORAGE__ENDPOINT_URL="http://127.0.0.1:9000/"
export MEMORY_MAP__STORAGE__ACCESS_KEY="memorymapdev"
export MEMORY_MAP__STORAGE__SECRET_KEY="memorymapdevsecret"
export MEMORY_MAP__STORAGE__BUCKET_NAME="memory-map"
export MEMORY_MAP__STORAGE__REGION="us-east-1"
export MEMORY_MAP__STORAGE__FORCE_PATH_STYLE="true"
export MEMORY_MAP__STORAGE__PRESIGNED_URL_TTL_SECONDS="604800"

export MEMORY_MAP__AUTH__COOKIE_SECRET="memory-map-local-e2e-cookie-secret-at-least-64-bytes-long-0001-extra"
export MEMORY_MAP__AUTH__ENABLE_REGISTRATION="true"

export MEMORY_MAP__SMTP__HOST="smtp.example.test"
export MEMORY_MAP__SMTP__USER="memory-map-e2e"
export MEMORY_MAP__SMTP__PASS="memory-map-e2e-password"
export MEMORY_MAP__SMTP__FROM="noreply@example.test"

export RUST_LOG="${E2E_RUST_LOG:-debug}"

export PROCESS_COMPOSE_PORT="${E2E_PROCESS_COMPOSE_PORT:-8080}"
# process-compose-flake exposes this project's wrapper as `default`.
export PROCESS_COMPOSE_BIN="${PROCESS_COMPOSE_BIN:-default}"
export E2E_LOG_DIR="${E2E_LOG_DIR:-e2e-logs}"
export PROCESS_COMPOSE_LOG="${PROCESS_COMPOSE_LOG:-$E2E_LOG_DIR/process-compose.log}"
