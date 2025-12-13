# Run PostgreSQL and Minio.
default:
    nix run ./devenv

# Run backend.
watch:
    bacon run -- --bin backend

# Run frontend. https://github.com/trunk-rs/trunk/issues/732#issuecomment-2391810077
serve:
    cd frontend; ping -c 1 8.8.8.8 && pnpm i --prefer-offline; trunk serve --skip-version-check --offline --open

# Regenerate frontend/graphql/schema.json
regenerate-schema:
    graphql-client introspect-schema http://localhost:8000 > frontend/graphql/schema.json
