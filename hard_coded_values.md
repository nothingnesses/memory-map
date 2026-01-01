# Hard-Coded Values Report

This document lists hard-coded values identified in the codebase that should be considered for refactoring into configuration files or environment variables.

## 1. Credentials & Secrets (High Priority)

These values pose a security risk if exposed or make it difficult to rotate secrets.

| File                  | Line | Value                        | Context                          |
| --------------------- | ---- | ---------------------------- | -------------------------------- |
| `backend/src/main.rs` | 185  | `"minioadmin", "minioadmin"` | MinIO StaticProvider credentials |
| `.env.example`        | 9    | `"password"`                 | SMTP Password                    |
| `.env.example`        | 11   | `"supersecretkey..."`        | Cookie Secret                    |

## 2. Network Configuration (URLs, IPs, Ports)

These values limit deployment flexibility (e.g., hardcoded to localhost).

| File                                                     | Line | Value                                    | Context             |
| -------------------------------------------------------- | ---- | ---------------------------------------- | ------------------- |
| `backend/src/main.rs`                                    | 182  | `"http://localhost:9000/"`               | MinIO Base URL      |
| `backend/src/main.rs`                                    | 228  | `"http://127.0.0.1:3000"`                | CORS Allowed Origin |
| `backend/src/main.rs`                                    | 255  | `"127.0.0.1:8000"`                       | Server Bind Address |
| `.env.example`                                           | 2    | `127.0.0.1`                              | Postgres Host       |
| `.env.example`                                           | 12   | `"http://localhost:3000"`                | Frontend URL        |
| `frontend/src/components/file_upload.rs`                 | 89   | `"http://localhost:8000/api/locations/"` | API Endpoint        |
| `frontend/src/graphql_queries/admin_update_user.rs`      | 22   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/change_email.rs`           | 22   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/change_password.rs`        | 19   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/config.rs`                 | 20   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/delete_s3_objects.rs`      | 21   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/login.rs`                  | 20   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/logout.rs`                 | 17   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/me.rs`                     | 23   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/register.rs`               | 20   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/request_password_reset.rs` | 20   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/reset_password.rs`         | 19   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/s3_object_by_id.rs`        | 26   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/s3_objects.rs`             | 26   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/update_s3_object.rs`       | 27   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/update_user_publicity.rs`  | 26   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/upsert_s3_object.rs`       | 26   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/src/graphql_queries/users.rs`                  | 22   | `"http://127.0.0.1:8000/"`               | GraphQL Endpoint    |
| `frontend/Trunk.toml`                                    | 37   | `["127.0.0.1"]`                          | Trunk Serve Address |

## 3. Application Constants

These are internal values that might need tuning but are less critical than secrets/network config.

| File                                | Line | Value           | Context                              |
| ----------------------------------- | ---- | --------------- | ------------------------------------ |
| `backend/src/main.rs`               | 42   | `10_000`        | `CACHE_MAX_CAPACITY`                 |
| `backend/src/main.rs`               | 44   | `600`           | `CACHE_TTL_SECONDS`                  |
| `backend/src/main.rs`               | 46   | `1024 * 1024`   | `GRAPHQL_BODY_LIMIT_BYTES`           |
| `backend/src/main.rs`               | 190  | `"memory-map"`  | `bucket_name` (Local Variable)       |
| `backend/src/lib.rs`                | 49   | `1_073_741_824` | `BODY_MAX_SIZE_LIMIT_BYTES`          |
| `frontend/src/components/header.rs` | 42   | `...`           | `HEADER_LAYER_CLASSES` (CSS classes) |
| `frontend/src/lib.rs`               | 42   | `100.0`         | `header_height` (Local Variable)     |
| `shared/src/lib.rs`                 | 1    | `[&str; 17]`    | `ALLOWED_MIME_TYPES`                 |

## 4. SQL Queries (Hard-coded Strings)

SQL queries are hard-coded in `backend/src/graphql/queries/mutation.rs`. While common in Rust without an ORM, they could be moved to separate SQL files (like `backend/migrations/`) or managed via a query builder if desired.

- `DELETE_OBJECTS_QUERY`
- `UPDATE_OBJECT_QUERY`
- `UPSERT_OBJECT_QUERY`

## How to Scan for More Values

A script `scan_hardcoded.sh` has been created to help you find these values in the future.

Usage:

```bash
./scan_hardcoded.sh
```

This script uses `git grep` to search for common patterns (constants, URLs, IPs, secrets) while respecting your `.gitignore` rules.
