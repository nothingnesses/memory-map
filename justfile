set dotenv-load

# Build mode configuration.
mode := env_var_or_default("BUILD_MODE", "debug")
release_flag := if mode == "release" { "--release" } else { "" }
log_prefix := if mode == "debug" { "RUST_LOG=debug" } else { "" }

# Display list of commands.
default:
    just -l

# Start PostgreSQL and Minio servers.
servers:
    nix run ./devenv

# Start backend server.
backend:
    {{log_prefix}} bacon run -- {{release_flag}} --bin backend

# Start frontend server. https://github.com/trunk-rs/trunk/issues/732#issuecomment-2391810077
frontend:
    cd frontend; ping -c 1 8.8.8.8 && pnpm i --prefer-offline; trunk serve {{release_flag}} --skip-version-check --offline --open

# Regenerate frontend/graphql/schema.json
regenerate-schema:
    graphql-client introspect-schema http://localhost:8000 > frontend/graphql/schema.json
