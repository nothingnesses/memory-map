# Memory Map Frontend

The frontend is a Leptos CSR application built with Trunk, UnoCSS, and Leaflet.

## Development

Run the frontend through the project task runner from the repository root:

```sh
just frontend
```

The app is served at `http://localhost:3000/`.

The frontend expects the backend to be available at the URL configured in
`frontend/public/config.json`.

## Build

To build the frontend:

```sh
just frontend-build
```

This runs `pnpm install --frozen-lockfile --prefer-offline` and then builds the
application through Trunk.

## GraphQL

GraphQL operations live in `frontend/graphql/`, and generated Rust query modules
live in `frontend/src/graphql_queries/`.

When the backend schema changes:

1. Start the backend with `just backend`.
2. Regenerate the schema with `just regenerate-schema`.
3. Rebuild or check the frontend.
