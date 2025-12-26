## Memory Map
Memory Map is a location-aware media archive that lets users upload photos, videos, and other files, automatically tagging them with time and GPS metadata, and visualizing them on an interactive world map.

Users can browse the map, click markers, and explore media galleries tied to real-world locations — creating a digital memory atlas.

## Features
Upload media files (images, videos, documents)

Automatic GPS location & timestamp tagging

Interactive world map with clickable memory pins

Gallery view for each map location

GraphQL API backend

Object storage powered by MinIO

Rust backend with modern Nix dev-shell

Fully browser-based frontend

## Tech Stack
Layer	Technology
Frontend	React / Next.js
Backend	Rust + GraphQL
Storage	MinIO
Database	PostgreSQL
Dev Env	Nix + direnv
Task Runner	Just

## Project Structure

memory-map/
│
├── .direnv/          # Direnv environment cache
├── backend/         # Rust GraphQL backend
├── data/            # Database & storage volumes
├── devenv/          # Nix development environment
├── frontend/        # React / Next.js frontend
├── shared/          # Shared utilities & types
├── .env.example     # Environment configuration template
├── Justfile         # Development commands
├── Cargo.toml       # Rust workspace configuration
├── Cargo.lock       # Rust dependency lock file
└── README.md        # Project documentation


Getting Started
1. Install dependencies

You’ll need:

Nix Package Manager

direnv

2. Clone & enter project
git clone https://github.com/your-org/memory-map.git
cd memory-map

3. Setup environment
cp .env.example .env
direnv allow


This installs all dependencies and auto-loads the development shell whenever you enter the folder.

4. Start database & storage
just

MinIO object storage becomes available at:

http://localhost:9001/login
Username: minioadmin
Password: minioadmin

5. Start backend

Open a new terminal:

just watch

Backend GraphQL playground:

http://localhost:8000/

6. Start frontend

Open another terminal:

just serve

Frontend app:

http://localhost:3000/

 Screenshots
Map View	Gallery View

	![Map View](./screenshots/map.png)

API
The backend exposes a GraphQL API at:

http://localhost:8000/

 Screenshots
GraphiQL 

	![GrtaphiQL View](./screenshots/graphiQL.png)


Use it to:

Upload files

Query memories by location

Retrieve gallery data

Contributing

We welcome contributions!
Please ensure:

direnv loads correctly

All services start via just

Frontend builds without errors

Code is formatted (cargo fmt, npm run lint)