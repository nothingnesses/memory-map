# Production Deployment

This project can run outside the Nix development service graph. Production
deployments should provide real runtime services and secrets through the
hosting platform, not through `devenv/flake.nix`.

## Runtime Shape

- Run the backend as a service or container with environment variables supplied
  by the production platform.
- Serve the frontend as static files built by Trunk.
- Provide `/config.json` at the frontend origin at deploy time.
- Use a real PostgreSQL instance or managed PostgreSQL service.
- Use AWS S3, RustFS, Garage, or another S3-compatible object store.
- Put TLS and a reverse proxy or load balancer in front of the public app.

The process-compose services in `devenv/flake.nix` are for local development
and CI only. Their deterministic credentials are not production secrets.

## Backend Environment

The backend reads configuration from two layered sources. An optional TOML file
is read first, then environment variables are layered on top (so the environment
always wins). The file is selected by the `MEMORY_MAP_CONFIG` environment
variable: set it to the file's path and the file is loaded and required; leave it
unset and the backend is configured purely from the environment (the default,
unchanged from previous releases). Secrets should not be committed: keep them in
a gitignored `config.toml` (copy `config.example.toml`, or run `just config`) or
supply them via environment variables.

In the TOML file, sections are tables and keys are flat within them, e.g.

```toml
[storage]
endpoint_url = "http://127.0.0.1:9000/"

[object_lifecycle]
storage_deletion_retry_seconds = 60
```

All variables share the `MEMORY_MAP__` prefix; `__` is both the prefix separator
and the path separator, so `MEMORY_MAP__STORAGE__ENDPOINT_URL` maps to
`config.storage.endpoint_url` (and to `storage.endpoint_url` in the TOML file).

Required database settings:

- `MEMORY_MAP__PG__DBNAME`
- `MEMORY_MAP__PG__HOST`
- `MEMORY_MAP__PG__PORT`

Required server settings:

- `MEMORY_MAP__SERVER__HOST`
- `MEMORY_MAP__SERVER__PORT`

Required SMTP settings:

- `MEMORY_MAP__SMTP__HOST`
- `MEMORY_MAP__SMTP__USER`
- `MEMORY_MAP__SMTP__PASS`
- `MEMORY_MAP__SMTP__FROM`

Required auth settings:

- `MEMORY_MAP__AUTH__COOKIE_SECRET`
- `MEMORY_MAP__AUTH__ENABLE_REGISTRATION`

Optional auth settings:

- `MEMORY_MAP__AUTH__COOKIE_SECURE` (default: infer from
  `MEMORY_MAP__FRONTEND__URL`)

Required frontend / CORS settings:

- `MEMORY_MAP__FRONTEND__URL`
- `MEMORY_MAP__CORS__ALLOWED_ORIGINS`

Required S3-compatible storage settings:

- `MEMORY_MAP__STORAGE__ENDPOINT_URL`
- `MEMORY_MAP__STORAGE__ACCESS_KEY`
- `MEMORY_MAP__STORAGE__SECRET_KEY`
- `MEMORY_MAP__STORAGE__BUCKET_NAME`

Optional S3 settings (defaults shown):

- `MEMORY_MAP__STORAGE__REGION` (default `us-east-1`)
- `MEMORY_MAP__STORAGE__FORCE_PATH_STYLE` (default `true`)
- `MEMORY_MAP__STORAGE__PRESIGNED_URL_TTL_SECONDS` (default `604800`)
- `MEMORY_MAP__STORAGE__PUBLIC_ENDPOINT_URL` (default: use
  `MEMORY_MAP__STORAGE__ENDPOINT_URL`)

Optional object lifecycle settings (defaults shown):

- `MEMORY_MAP__OBJECT_LIFECYCLE__PENDING_UPLOAD_TIMEOUT_SECONDS` (default `3600`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__UPLOAD_MAX_FILE_SIZE_BYTES` (default
  `1073741824`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__UPLOAD_PART_SIZE_BYTES` (default `8388608`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__UPLOAD_MAX_PART_COUNT` (default `10000`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__UPLOAD_SESSION_TTL_SECONDS` (default `3600`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__UPLOAD_SESSION_CLEANUP_RETRY_SECONDS` (default
  `60`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__UPLOAD_SESSION_CLEANUP_LEASE_SECONDS` (default
  `300`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__UPLOAD_SESSION_CLEANUP_MAX_ATTEMPTS` (default
  `10`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__UPLOAD_SESSION_CLEANUP_BATCH_SIZE` (default
  `1000`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__STORAGE_DELETION_RETRY_SECONDS` (default `60`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__STORAGE_DELETION_LEASE_SECONDS` (default `300`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__MAINTENANCE_INTERVAL_SECONDS` (default `30`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__STORAGE_DELETION_BATCH_SIZE` (default `1000`)
- `MEMORY_MAP__OBJECT_LIFECYCLE__STORAGE_DELETION_MAX_ATTEMPTS` (default `10`)

Optional email outbox settings (defaults shown):

- `MEMORY_MAP__EMAIL_OUTBOX__RETRY_SECONDS` (default `60`)
- `MEMORY_MAP__EMAIL_OUTBOX__LEASE_SECONDS` (default `300`)
- `MEMORY_MAP__EMAIL_OUTBOX__WORKER_INTERVAL_SECONDS` (default `30`)
- `MEMORY_MAP__EMAIL_OUTBOX__BATCH_SIZE` (default `100`)
- `MEMORY_MAP__EMAIL_OUTBOX__MAX_ATTEMPTS` (default `10`)

`MEMORY_MAP__AUTH__COOKIE_SECRET`, `MEMORY_MAP__SMTP__PASS`,
`MEMORY_MAP__STORAGE__ACCESS_KEY`, and `MEMORY_MAP__STORAGE__SECRET_KEY` must
come from production secret management. Do not copy values from `.env.example`
or `devenv/flake.nix` into production.

`MEMORY_MAP__STORAGE__PRESIGNED_URL_TTL_SECONDS` must be between `1` and
`604800`.

`MEMORY_MAP__AUTH__COOKIE_SECURE` overrides whether login/logout cookies use
the `Secure` attribute. Leave it unset to infer from
`MEMORY_MAP__FRONTEND__URL`; set it to `true` when TLS is terminated by a
proxy but the configured frontend URL is not `https`.

`MEMORY_MAP__STORAGE__PUBLIC_ENDPOINT_URL` signs browser-facing object URLs
when the backend reaches storage through a private endpoint but browsers need a
different public endpoint.

`MEMORY_MAP__OBJECT_LIFECYCLE__PENDING_UPLOAD_TIMEOUT_SECONDS` controls when an
unfinalized upload is treated as failed and moved into cleanup. It should be
comfortably longer than the longest expected object upload.

`MEMORY_MAP__OBJECT_LIFECYCLE__UPLOAD_PART_SIZE_BYTES` must be at least
`5242880`, the S3 multipart minimum for non-final parts. The maximum file size
must fit within `UPLOAD_MAX_PART_COUNT` parts.

`MEMORY_MAP__OBJECT_LIFECYCLE__UPLOAD_SESSION_TTL_SECONDS` controls how long a
direct-upload session can be completed before it is eligible for reconciliation.

Expired direct-upload sessions are reconciled by the backend worker. It aborts
incomplete multipart uploads, removes pending metadata after successful aborts,
and moves completed-object orphans into the storage-deletion outbox.

`MEMORY_MAP__OBJECT_LIFECYCLE__STORAGE_DELETION_MAX_ATTEMPTS` bounds how many
times a failing storage deletion is retried before it is parked. Rows past the
cap remain in `object_storage_deletions` with their `last_error` populated for
operator triage, but are no longer reclaimed by the worker.

Password-reset emails are sent by a backend email outbox worker. Requests insert
reset tokens and email rows in one database transaction; SMTP delivery happens
after commit. `MEMORY_MAP__EMAIL_OUTBOX__MAX_ATTEMPTS` bounds delivery retries.
Rows past the cap remain in `email_outbox` with their `last_error` populated
for operator triage, but are no longer reclaimed by the worker.

## Frontend Runtime Config

The frontend is a static client-side rendered app. At runtime it fetches
`/config.json` from the same origin as the frontend. The concrete runtime file
`frontend/public/config.json` is intentionally ignored by Git.

Production must provide a concrete `/config.json` during deployment. The
frontend build recipe requires `frontend/public/config.json` to exist, but it
does not create the local example automatically:

```json
{
	"api_url": "https://api.example.com"
}
```

`api_url` is public runtime configuration, not a secret. It should point to the
public backend GraphQL/API origin used by browsers.

For local development, run `just frontend-config` to create
`frontend/public/config.json` from `frontend/config.example.json`. The example
uses the local backend API URL, `http://127.0.0.1:8000`, which is not suitable
for production. Do not deploy a frontend build with that local runtime config.

## PostgreSQL

Production should use a real PostgreSQL instance or a managed PostgreSQL
service. Do not use local `data/postgres` state from process-compose.

The backend runs database migrations on startup using the configured database
connection. The database user must have the privileges needed to run those
migrations.

## Object Storage

The backend uses the AWS Rust S3 SDK against the configured S3-compatible
endpoint. The app needs:

- object upload
- multipart object upload and abort
- object metadata lookup
- presigned GET URLs
- presigned multipart upload-part URLs
- content-type metadata
- bulk object delete

Path-style guidance:

- Local RustFS uses `MEMORY_MAP__STORAGE__FORCE_PATH_STYLE=true`.
- AWS S3 normally uses `MEMORY_MAP__STORAGE__FORCE_PATH_STYLE=false`.
- Other S3-compatible services should follow the provider's requirements.

The configured bucket must be usable by the configured credentials. The backend
verifies bucket access on startup but does not create buckets at runtime. The
local and CI bootstrap helper can create or verify a bucket for RustFS, but
production bucket lifecycle should be managed intentionally by the deployment.
Buckets used by the browser direct-upload flow must allow CORS `PUT` requests
from the public frontend origin, allow the signed request headers used by
presigned upload-part URLs, and expose the `ETag` response header so the
frontend can complete multipart uploads. The local RustFS bootstrap applies
this policy from `MEMORY_MAP__FRONTEND__URL` and
`MEMORY_MAP__CORS__ALLOWED_ORIGINS`; production bucket CORS should be managed
with the bucket infrastructure.

Database object deletes first mark metadata rows as delete-pending and enqueue
cleanup by the immutable storage key, not the user-visible object name. A
backend worker claims cleanup rows in bounded batches, deletes the blobs, and
then removes the delete-pending metadata rows. If storage deletion fails, the
queue row remains for a later retry instead of losing track of the blob cleanup
work. Pending rows without upload sessions are also moved into the same cleanup
path after the configured timeout.

## Reverse Proxy And TLS

Serve the frontend over HTTPS in production.

Route backend GraphQL traffic to the backend service. The backend serves
GraphQL at `/`; browser object bytes upload directly to the configured
S3-compatible storage endpoint through presigned multipart URLs.

Set `MEMORY_MAP__FRONTEND__URL` and `MEMORY_MAP__CORS__ALLOWED_ORIGINS` to the
public frontend origin. For example:

```sh
MEMORY_MAP__FRONTEND__URL=https://memory-map.example.com
MEMORY_MAP__CORS__ALLOWED_ORIGINS=https://memory-map.example.com
```

Keep browser traffic on HTTPS so authenticated cookies and presigned media URL
access are not exposed over plaintext connections.

## Out Of Scope

This repository does not currently define a production Docker image, systemd
unit, NixOS module, Kubernetes manifest, or hosting-specific deployment
artifact. Add those only after choosing a concrete production platform.
