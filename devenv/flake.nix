{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.1";
    nixpkgs-unstable.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix-monthly = {
      url = "github:nix-community/fenix/monthly";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
    systems.url = "github:nix-systems/default";
    process-compose-flake.url = "github:Platonic-Systems/process-compose-flake";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } (
      top@{
        config,
        withSystem,
        moduleWithSystem,
        ...
      }:
      {
        imports = [
          # Optional: use external flake logic, e.g.
          # inputs.foo.flakeModules.default
          inputs.process-compose-flake.flakeModule
          inputs.flake-parts.flakeModules.easyOverlay
        ];
        # flake = {
        #   # Put your original flake attributes here.
        # };
        # systems for which you want to build the `perSystem` attributes
        systems = import inputs.systems;
        perSystem =
          {
            self',
            pkgs,
            config,
            lib,
            final,
            system,
            ...
          }:
          let
            rustToolchain =
              with inputs.fenix.packages.${pkgs.stdenv.hostPlatform.system};
              combine [
                stable.clippy
                stable.rustc
                stable.cargo
                inputs.fenix-monthly.packages.${pkgs.stdenv.hostPlatform.system}.latest.rustfmt
                stable.rust-src
                # For Leptos
                targets.wasm32-unknown-unknown.stable.rust-std
              ];
            appRustPlatform = pkgs.makeRustPlatform {
              cargo = rustToolchain;
              rustc = rustToolchain;
            };
            appSource =
              let
                root = toString ./..;
              in
              lib.cleanSourceWith {
                src = ./..;
                filter =
                  path: type:
                  let
                    rel = lib.removePrefix "${root}/" (toString path);
                  in
                  !(lib.hasPrefix "target/" rel)
                  && !(lib.hasPrefix "data/" rel)
                  && !(lib.hasPrefix "frontend/node_modules/" rel)
                  && !(lib.hasPrefix "frontend/dist/" rel)
                  && rel != "process-compose.log"
                  && rel != "frontend/public/config.json"
                  && !(lib.hasPrefix "e2e-logs/" rel)
                  && !(lib.hasPrefix "frontend/playwright-report/" rel)
                  && !(lib.hasPrefix "frontend/test-results/" rel)
                  && !(lib.hasPrefix "frontend/blob-report/" rel);
              };
            rustfsPackage = pkgs.callPackage ./packages/rustfs.nix { };
            storageBootstrap = appRustPlatform.buildRustPackage {
              pname = "memory-map-storage-bootstrap";
              version = "0.1.0";
              src = appSource;
              cargoLock.lockFile = ../Cargo.lock;
              cargoBuildFlags = [
                "-p"
                "backend"
                "--bin"
                "memory-map-storage-bootstrap"
              ];
              doCheck = false;
              nativeBuildInputs = [ pkgs.pkg-config ];
              buildInputs = [ pkgs.openssl ];
              meta.mainProgram = "memory-map-storage-bootstrap";
            };

            localPostgres = {
              dbName = "db";
              host = "127.0.0.1";
              port = 5432;
              dataDir = "data/postgres";
            };
            localRustfs = {
              dataDir = "data/rustfs";
              apiAddress = "127.0.0.1:9000";
              consoleAddress = "127.0.0.1:9001";
              consoleEnabled = true;
              healthEndpointEnabled = true;
            };
            localS3 = {
              endpointUrl = "http://${localRustfs.apiAddress}";
              region = "us-east-1";
              bucketName = "memory-map";
              forcePathStyle = true;
              presignedUrlTtlSeconds = 604800;
              # Deterministic local/CI credentials for the RustFS service.
              # These are not production secrets.
              accessKey = "memorymapdev";
              secretKey = "memorymapdevsecret";
            };
            postgresPackage = pkgs.postgresql.withPackages (postgresExtensions: [
              postgresExtensions.postgis
            ]);
            postgresHbaConf = pkgs.writeText "pg_hba.conf" ''
              # Generated by Nix
              local all all trust
              host all all ${localPostgres.host}/32 trust
              host all all ::1/128 trust
              local replication all trust
              host replication all ${localPostgres.host}/32 trust
              host replication all ::1/128 trust
            '';
            postgresInit = pkgs.writeShellApplication {
              name = "memory-map-postgres-init";
              runtimeInputs = [
                postgresPackage
                pkgs.coreutils
              ];
              text = ''
                set -euo pipefail

                PGDATA="$(readlink -m "${localPostgres.dataDir}")"
                export PGDATA
                mkdir -p "$PGDATA"

                if [ -s "$PGDATA/PG_VERSION" ]; then
                  echo "PostgreSQL data directory already initialized: $PGDATA"
                  exit 0
                fi

                initdb --locale=C --encoding=UTF8 -D "$PGDATA"
              '';
            };
            postgresStart = pkgs.writeShellApplication {
              name = "memory-map-postgres-start";
              runtimeInputs = [
                postgresPackage
                pkgs.coreutils
              ];
              text = ''
                set -euo pipefail

                PGDATA="$(readlink -m "${localPostgres.dataDir}")"
                export PGDATA

                exec postgres \
                  -D "$PGDATA" \
                  -c listen_addresses=${localPostgres.host} \
                  -c port=${toString localPostgres.port} \
                  -c hba_file=${postgresHbaConf} \
                  -c unix_socket_directories=
              '';
            };
            postgresBootstrap = pkgs.writeShellApplication {
              name = "memory-map-postgres-bootstrap";
              runtimeInputs = [
                postgresPackage
                pkgs.gnugrep
              ];
              text = ''
                set -euo pipefail

                export PGHOST=${localPostgres.host}
                export PGPORT=${toString localPostgres.port}
                export PGDATABASE=postgres
                export PGCONNECT_TIMEOUT=5

                if ! psql -v ON_ERROR_STOP=1 -tAc "SELECT 1 FROM pg_database WHERE datname = '${localPostgres.dbName}'" | grep -qx 1; then
                  createdb "${localPostgres.dbName}"
                fi

                psql -v ON_ERROR_STOP=1 -tAc "SELECT name FROM pg_available_extensions WHERE name = 'postgis'" | grep -qx postgis
              '';
            };
            postgresTest = pkgs.writeShellApplication {
              name = "memory-map-postgres-test";
              runtimeInputs = [
                postgresPackage
                pkgs.gnugrep
              ];
              text = ''
                set -euo pipefail

                psql -h ${localPostgres.host} -p ${toString localPostgres.port} -d ${localPostgres.dbName} -v ON_ERROR_STOP=1 -c 'SELECT version();'
                psql -h ${localPostgres.host} -p ${toString localPostgres.port} -d ${localPostgres.dbName} -v ON_ERROR_STOP=1 -tAc "SELECT name FROM pg_available_extensions WHERE name = 'postgis'" | grep -qx postgis
              '';
            };

            rustfsStart = pkgs.writeShellApplication {
              name = "memory-map-rustfs-start";
              runtimeInputs = [
                rustfsPackage
                pkgs.coreutils
              ];
              text = ''
                set -euo pipefail

                data_dir="$(readlink -m "${localRustfs.dataDir}")"
                mkdir -p "$data_dir"
                exec rustfs server "$data_dir"
              '';
            };
            rustfsBootstrap = pkgs.writeShellApplication {
              name = "memory-map-rustfs-bootstrap";
              runtimeInputs = [ storageBootstrap ];
              text = ''
                set -euo pipefail

                export S3_ENDPOINT_URL="${localS3.endpointUrl}"
                export S3_ACCESS_KEY="${localS3.accessKey}"
                export S3_SECRET_KEY="${localS3.secretKey}"
                export S3_BUCKET_NAME="${localS3.bucketName}"
                export S3_REGION="${localS3.region}"
                export S3_FORCE_PATH_STYLE=${lib.boolToString localS3.forcePathStyle}
                export S3_PRESIGNED_URL_TTL_SECONDS=${toString localS3.presignedUrlTtlSeconds}

                exec memory-map-storage-bootstrap
              '';
            };

            treefmtEval = inputs.treefmt-nix.lib.evalModule pkgs {
              # Cargo.toml lives at the repo root (one level above devenv/).
              projectRootFile = "Cargo.toml";
              programs = {
                nixfmt.enable = true;
                rustfmt = {
                  enable = true;
                  package = rustToolchain;
                };
                prettier = {
                  enable = true;
                  includes = [
                    "*.md"
                    "*.yml"
                    "*.yaml"
                  ];
                };
              };
              settings.formatter.tombi = {
                command = "${inputs.nixpkgs-unstable.legacyPackages.${system}.tombi}/bin/tombi";
                options = [
                  "format"
                  "--offline"
                ];
                includes = [ "*.toml" ];
              };
              settings.global.excludes = [ "frontend/pnpm-lock.yaml" ];
            };

            pre-commit-check = inputs.git-hooks.lib.${system}.run {
              src = ./..;
              hooks = {
                treefmt = {
                  enable = true;
                  package = treefmtEval.config.build.wrapper;
                };
                # These run on pre-push because whole-project tools do not mix
                # well with pre-commit's partial-file staging behaviour.
                clippy = {
                  enable = true;
                  entry = "${pkgs.just}/bin/just clippy";
                  pass_filenames = false;
                  always_run = true;
                  stages = [ "pre-push" ];
                };
                cargo-doc = {
                  enable = true;
                  entry = "${pkgs.just}/bin/just doc";
                  pass_filenames = false;
                  always_run = true;
                  stages = [ "pre-push" ];
                };
              };
            };
          in
          {
            # Recommended: move all package definitions here.
            # e.g. (assuming you have a nixpkgs input)
            # packages.foo = pkgs.callPackage ./foo/package.nix { };
            # packages.bar = pkgs.callPackage ./bar/package.nix {
            #   foo = config.packages.foo;
            # };

            formatter = treefmtEval.config.build.wrapper;

            checks = {
              formatting = treefmtEval.config.build.check self'.self;
              inherit pre-commit-check;
            };

            _module.args.pkgs = import inputs.nixpkgs {
              inherit system;
              config.allowUnfree = true;
              overlays = [
                (final: _prev: {
                  unstable = import inputs.nixpkgs-unstable {
                    inherit (final) system;
                    config.allowUnfree = true;
                  };
                })
              ];
            };

            overlayAttrs = {
              inherit (config.packages) rustToolchain rustfs storageBootstrap;
            };

            packages.rustToolchain = rustToolchain;
            packages.rustfs = rustfsPackage;
            packages.storageBootstrap = storageBootstrap;

            # `process-compose.foo` will add a flake package output called "foo".
            # Therefore, this will add a default package that you can build using
            # `nix build` and run using `nix run`.
            process-compose."default" = {
              settings = {
                ordered_shutdown = true;
                processes = {
                  pg1-init.command = postgresInit;
                  pg1 = {
                    command = postgresStart;
                    depends_on."pg1-init".condition = "process_completed_successfully";
                    shutdown = {
                      signal = 2;
                      timeout_seconds = 10;
                    };
                    readiness_probe = {
                      exec.command = "${postgresPackage}/bin/pg_isready -h ${localPostgres.host} -p ${toString localPostgres.port} -d postgres";
                      initial_delay_seconds = 1;
                      period_seconds = 2;
                      timeout_seconds = 5;
                      success_threshold = 1;
                      failure_threshold = 10;
                    };
                    availability = {
                      restart = "on_failure";
                      max_restarts = 5;
                    };
                  };
                  pg1-bootstrap = {
                    command = postgresBootstrap;
                    depends_on."pg1".condition = "process_healthy";
                  };
                  pgweb = {
                    environment.PGWEB_DATABASE_URL = "postgres://${localPostgres.host}:${toString localPostgres.port}/${localPostgres.dbName}";
                    command = pkgs.pgweb;
                    depends_on."pg1-bootstrap".condition = "process_completed_successfully";
                  };
                  pg1-test = {
                    command = postgresTest;
                    depends_on."pg1-bootstrap".condition = "process_completed_successfully";
                  };
                  rustfs = {
                    command = rustfsStart;
                    environment = {
                      RUSTFS_ACCESS_KEY = localS3.accessKey;
                      RUSTFS_SECRET_KEY = localS3.secretKey;
                      RUSTFS_ADDRESS = localRustfs.apiAddress;
                      RUSTFS_CONSOLE_ENABLE = lib.boolToString localRustfs.consoleEnabled;
                      RUSTFS_CONSOLE_ADDRESS = localRustfs.consoleAddress;
                      RUSTFS_REGION = localS3.region;
                      RUSTFS_HEALTH_ENDPOINT_ENABLE = lib.boolToString localRustfs.healthEndpointEnabled;
                    };
                    shutdown = {
                      signal = 15;
                      timeout_seconds = 5;
                    };
                    readiness_probe = {
                      exec.command = lib.getExe rustfsBootstrap;
                      initial_delay_seconds = 2;
                      period_seconds = 5;
                      timeout_seconds = 8;
                      success_threshold = 1;
                      failure_threshold = 6;
                    };
                    availability = {
                      restart = "on_failure";
                      max_restarts = 3;
                    };
                  };
                };
              };
            };

            devShells.default = pkgs.mkShell {
              # Alias for `nativeBuildInputs`
              # https://discourse.nixos.org/t/difference-between-buildinputs-and-packages-in-mkshell/60598/10
              packages = [
                pkgs.bashInteractive
                config.packages.rustToolchain
                postgresPackage
                pkgs.openssl
                pkgs.pkg-config
                pkgs.cargo-deny
                pkgs.cargo-edit
                pkgs.bacon
                pkgs.rust-analyzer
                pkgs.gh
                # Stable didn't yet have cargo-generate, so we're using unstable here
                pkgs.unstable.cargo-generate
                pkgs.just
                pkgs.curl
                pkgs.process-compose
                pkgs.pnpm
                pkgs.nodejs-slim
                pkgs.playwright-driver
                pkgs.playwright-driver.browsers
                pkgs.graphql-client
                # For Leptos
                pkgs.leptosfmt
                pkgs.trunk
                # https://github.com/trunk-rs/trunk/issues/732#issuecomment-2391810077
                pkgs.dart-sass
                pkgs.unstable.wasm-bindgen-cli_0_2_118
                # Needed for building in release mode
                pkgs.binaryen
                # For link checking in markdown
                pkgs.lychee
                # pkgs.tailwindcss
                # For finding function calls
                pkgs.ast-grep
                # For ASCII-only lint check in `just doc`
                pkgs.ripgrep
              ];

              env = {
                # Keep Cargo subcommands on the flake-selected toolchain even
                # when the parent shell exports stale toolchain paths.
                CARGO = "${config.packages.rustToolchain}/bin/cargo";
                RUSTC = "${config.packages.rustToolchain}/bin/rustc";
                LD_LIBRARY_PATH = lib.makeLibraryPath [
                  pkgs.openssl
                ];
                PLAYWRIGHT_BROWSERS_PATH = "${pkgs.playwright-driver.browsers}";
                PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";
                # Required by rust-analyzer
                RUST_SRC_PATH = "${config.packages.rustToolchain}/lib/rustlib/src/rust/library";
              };

              inherit (pre-commit-check) shellHook;
            };
          };
      }
    );
}
