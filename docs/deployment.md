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

The backend reads configuration from environment variables.

Required database settings:

- `PG__DBNAME`
- `PG__HOST`
- `PG__PORT`

Required SMTP settings:

- `SMTP_HOST`
- `SMTP_USER`
- `SMTP_PASS`
- `SMTP_FROM`

Required app settings:

- `COOKIE_SECRET`
- `FRONTEND_URL`
- `SERVER_HOST`
- `SERVER_PORT`
- `CORS_ALLOWED_ORIGINS`

Required S3-compatible storage settings:

- `S3_ENDPOINT_URL`
- `S3_ACCESS_KEY`
- `S3_SECRET_KEY`
- `S3_BUCKET_NAME`
- `S3_REGION`
- `S3_FORCE_PATH_STYLE`
- `S3_PRESIGNED_URL_TTL_SECONDS`

`COOKIE_SECRET`, `SMTP_PASS`, `S3_ACCESS_KEY`, and `S3_SECRET_KEY` must come
from production secret management. Do not copy values from `.env.example` or
`devenv/flake.nix` into production.

`S3_PRESIGNED_URL_TTL_SECONDS` must be between `1` and `604800`.

## Frontend Runtime Config

The frontend is a static client-side rendered app. At runtime it fetches
`/config.json` from the same origin as the frontend.

Production must provide a concrete `/config.json` during deployment:

```json
{
	"api_url": "https://api.example.com"
}
```

`api_url` is public runtime configuration, not a secret. It should point to the
public backend GraphQL/API origin used by browsers.

The local development API URL, `http://127.0.0.1:8000`, is not suitable for
production. Do not deploy a frontend build with that runtime config.

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
- object metadata lookup
- presigned GET URLs
- content-type metadata
- bulk object delete

Path-style guidance:

- Local RustFS uses `S3_FORCE_PATH_STYLE=true`.
- AWS S3 normally uses `S3_FORCE_PATH_STYLE=false`.
- Other S3-compatible services should follow the provider's requirements.

The configured bucket must be usable by the configured credentials. The backend
storage helper can create or verify a bucket for local and CI RustFS, but
production bucket lifecycle should be managed intentionally by the deployment.

## Reverse Proxy And TLS

Serve the frontend over HTTPS in production.

Route backend GraphQL and upload traffic to the backend service. The backend
serves GraphQL at `/` and uploads at `/api/locations/`.

Set `FRONTEND_URL` and `CORS_ALLOWED_ORIGINS` to the public frontend origin.
For example:

```sh
FRONTEND_URL=https://memory-map.example.com
CORS_ALLOWED_ORIGINS=https://memory-map.example.com
```

Keep browser traffic on HTTPS so authenticated cookies and presigned media URL
access are not exposed over plaintext connections.

## Out Of Scope

This repository does not currently define a production Docker image, systemd
unit, NixOS module, Kubernetes manifest, or hosting-specific deployment
artifact. Add those only after choosing a concrete production platform.
