# Display list of commands.
default:
    just -l

# Start PostgreSQL and Minio servers.
servers:
    nix run ./devenv

# Start backend server.
backend:
    bacon run -- --bin backend

# Start frontend server. https://github.com/trunk-rs/trunk/issues/732#issuecomment-2391810077
frontend:
    cd frontend; ping -c 1 8.8.8.8 && pnpm i --prefer-offline; trunk serve --skip-version-check --offline --open

# Regenerate frontend/graphql/schema.json
regenerate-schema:
    graphql-client introspect-schema http://localhost:8000 > frontend/graphql/schema.json
