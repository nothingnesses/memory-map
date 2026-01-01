# Refactoring Plan: Hard-Coded Values

This plan outlines the steps to remove hard-coded values identified in `hard_coded_values.md`.

## 1. Backend Configuration (Priority: High)

**Goal:** Move secrets and environment-specific network config to `.env` and `Config` struct.

### Steps:
1.  **Update `.env.example`**: Add missing keys:
    *   `MINIO_ACCESS_KEY`, `MINIO_SECRET_KEY`, `MINIO_URL`
    *   `SERVER_HOST`, `SERVER_PORT`
    *   `CORS_ALLOWED_ORIGINS`
2.  **Update `backend/src/lib.rs` (Config struct)**:
    *   **Reuse Existing Struct**: Add fields to the existing `Config` struct to match the new env vars.
    *   Leverage the existing `config` crate to load these values (merging environment variables with defaults).
3.  **Refactor `backend/src/main.rs`**:
    *   Replace `StaticProvider::new("minioadmin", ...)` with `cfg.minio_access_key`.
    *   Replace `TcpListener::bind(...)` with `format!("{}:{}", cfg.server_host, cfg.server_port)`.
    *   Replace CORS origins with `cfg.cors_allowed_origins`.

## 2. Backend Constants (Priority: Medium)

**Goal:** Centralize internal tuning parameters.

### Steps:
1.  **Create `backend/src/constants.rs`**:
    *   Move `CACHE_MAX_CAPACITY`, `CACHE_TTL_SECONDS`, `GRAPHQL_BODY_LIMIT_BYTES` here.
2.  **Update `backend/src/main.rs`**:
    *   Import constants from `constants.rs`.

## 3. Frontend Configuration (Priority: High)

**Goal:** Allow frontend to talk to any backend by fetching configuration at runtime.

### Implementation Details (Fetch `config.json`):

1.  **The Config File**:
    *   Create `frontend/public/config.json`:
        ```json
        {
          "api_url": "http://localhost:8000"
        }
        ```
    *   *Note: In production, this file will be edited on the server to point to the real API URL (e.g., `https://api.memory-map.com`).*

2.  **Frontend Code (`frontend/src/main.rs`)**:
    *   **Define Struct**: Create a new Rust struct `AppConfig` matching the JSON.
        *   *Reasoning*: We cannot reuse the existing `PublicConfig` (from GraphQL) because we need this URL *before* we can make any GraphQL requests.
    *   **Fetch on Startup**: Before calling `mount_to_body`, use `reqwest` to fetch `/config.json`.
        *   Example: `reqwest::get("/config.json").await?.json::<AppConfig>().await?`
    *   **Store in Context**: Pass this config struct to the app via `provide_context`.

3.  **Usage**:
    *   Update API calls (e.g., in `frontend/src/graphql_queries/`) to use `use_context::<AppConfig>()` to get the `api_url` instead of hardcoded strings.

## 4. SQL Queries (Priority: Low)

**Goal:** Clean up `mutation.rs`.

### Steps:
1.  **Create `backend/src/db/queries.rs`**:
    *   Move long SQL string constants here.

## 5. Supporting Custom Domains (Production Hosting)

To support hosting on a domain (e.g., `https://memory-map.com`) without ports:

1.  **Frontend**: With the `config.json` approach, you simply set `"api_url": "https://memory-map.com"` (or `https://api.memory-map.com`) in the production `config.json`.
2.  **Reverse Proxy**: Use Nginx/Caddy to serve the frontend and proxy API requests if needed.
3.  **Backend**: Configure `SERVER_HOST` to listen on an internal port.
