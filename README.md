# Memory Map

Memory Map is a location-aware media archive that allows users to upload photos, videos, and audio files.

The list of supported file types is defined in the [allowed files list](https://github.com/nothingnesses/memory-map/blob/main/shared/src/lib.rs).

Time and location metadata are manually provided by users and are used to visualise uploaded media on an interactive world map.

Users can browse the map, click markers and explore media galleries tied to real-world locations - creating a digital memory atlas.

## Features

- Upload media files (images, videos and audio files)
- Manual GPS location & timestamp tagging
- Interactive world map with clickable memory pins
- Gallery view for each map location

## Tech Stack

| Layer                   | Technology                                                |
| ----------------------- | --------------------------------------------------------- |
| Frontend                | [Leptos](https://leptos.dev/)                             |
|                         | [UnoCSS](https://unocss.dev/)                             |
| Backend                 | [Axum](https://github.com/tokio-rs/axum)                  |
|                         | [GraphQL](https://graphql.org)                            |
| Storage                 | [MinIO](https://min.io)                                   |
| Database                | [PostgreSQL](https://www.postgresql.org)                  |
| Development Environment | [Nix package manager](https://nixos.org)                  |
|                         | [nix-direnv](https://github.com/nix-community/nix-direnv) |
| Task Runner             | [Just](https://github.com/casey/just)                     |

## Project Structure

```
memory-map/
│
├── .direnv/         # Direnv environment cache
├── backend/         # Axum and GraphQL backend
├── data/            # Database and storage volumes
├── devenv/          # Nix development environment
├── frontend/        # Leptos and UnoCSS frontend
├── shared/          # Shared utilities & types
├── .env.example     # Environment configuration template
├── justfile         # Development commands
├── Cargo.toml       # Rust workspace configuration
├── Cargo.lock       # Rust dependency lock file
└── README.md        # Project documentation
```

## Getting Started

1. Install dependencies

You’ll need:

- [Nix Package Manager](https://nixos.org/download/)
- [nix-direnv](https://github.com/nix-community/nix-direnv?tab=readme-ov-file#installation)

2. Clone & enter project (you only need to do this step once)

```sh
git clone https://github.com/nothingnesses/memory-map.git
cd memory-map
```

3. Setup environment (you only need to do this step once)

```sh
cp .env.example .env
direnv allow
```

This installs all dependencies and auto-loads the development shell whenever you enter the directory.

You can optionally configure the build mode by editing `.env`:

- `BUILD_MODE="debug"` (default): Faster compilation, includes debug info.
- `BUILD_MODE="release"`: Optimised build, smaller binaries, slower compilation.

4. Start database & storage

```sh
just servers
```

MinIO object storage becomes available at:

http://localhost:9001/login

Username: minioadmin

Password: minioadmin

5. Start backend

In another shell, run:

```sh
just backend
```

Backend GraphQL playground:

http://localhost:8000/

6. Start frontend

In another shell, run:

```sh
just frontend
```

Frontend app:

http://localhost:3000/

Screenshots
Map View Gallery View

    ![Map View](./screenshots/map.png)

API
The backend exposes a GraphQL API at:

http://localhost:8000/

Screenshots
GraphiQL

    ![GraphiQL View](./screenshots/graphiQL.png)

Use it to:

- Query memories by location
- Retrieve gallery data

## Contributing

We welcome contributions!
Please ensure:

- direnv loads correctly
- All services start via `just`
- Frontend builds without errors
- Code is formatted (`cargo fmt`)
