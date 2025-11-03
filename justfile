# Run PostgreSQL and Minio.
default:
    nix run ./devenv

# Run backend.
watch:
    bacon run -- --bin backend

# Run frontend. https://github.com/trunk-rs/trunk/issues/732#issuecomment-2391810077
serve:
    cd frontend; trunk serve --skip-version-check --offline --open
