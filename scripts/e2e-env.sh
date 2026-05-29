#!/usr/bin/env bash

export E2E_FRONTEND_URL="${E2E_FRONTEND_URL:-http://127.0.0.1:3000}"
export E2E_BACKEND_URL="${E2E_BACKEND_URL:-http://127.0.0.1:8000}"

export FRONTEND_URL="$E2E_FRONTEND_URL"
export CORS_ALLOWED_ORIGINS="$E2E_FRONTEND_URL"
export SERVER_HOST="127.0.0.1"
export SERVER_PORT="8000"

export PG__DBNAME="db"
export PG__HOST="127.0.0.1"
export PG__PORT="5432"

export S3_ENDPOINT_URL="http://127.0.0.1:9000/"
export S3_ACCESS_KEY="memorymapdev"
export S3_SECRET_KEY="memorymapdevsecret"
export S3_BUCKET_NAME="memory-map"
export S3_REGION="us-east-1"
export S3_FORCE_PATH_STYLE="true"
export S3_PRESIGNED_URL_TTL_SECONDS="604800"

export COOKIE_SECRET="memory-map-local-e2e-cookie-secret-at-least-64-bytes-long-0001-extra"
export SMTP_HOST="smtp.example.test"
export SMTP_USER="memory-map-e2e"
export SMTP_PASS="memory-map-e2e-password"
export SMTP_FROM="noreply@example.test"
export ENABLE_REGISTRATION="true"
export RUST_LOG="${E2E_RUST_LOG:-debug}"

export PROCESS_COMPOSE_PORT="${E2E_PROCESS_COMPOSE_PORT:-8080}"
# process-compose-flake exposes this project's wrapper as `default`.
export PROCESS_COMPOSE_BIN="${PROCESS_COMPOSE_BIN:-default}"
export E2E_LOG_DIR="${E2E_LOG_DIR:-e2e-logs}"
export PROCESS_COMPOSE_LOG="${PROCESS_COMPOSE_LOG:-$E2E_LOG_DIR/process-compose.log}"
