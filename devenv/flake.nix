{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.1";
    nixpkgs-minio.url = "github:NixOS/nixpkgs/e6f23dc08d3624daab7094b701aa3954923c6bbb";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
    systems.url = "github:nix-systems/default";
    process-compose-flake.url = "github:Platonic-Systems/process-compose-flake";
    services-flake.url = "github:juspay/services-flake";
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
          {
            # Recommended: move all package definitions here.
            # e.g. (assuming you have a nixpkgs input)
            # packages.foo = pkgs.callPackage ./foo/package.nix { };
            # packages.bar = pkgs.callPackage ./bar/package.nix {
            #   foo = config.packages.foo;
            # };

            _module.args.pkgs = import inputs.nixpkgs {
              inherit system;
              config.allowUnfree = true;
              overlays = [
                (final: _prev: {
                  minio = import inputs.nixpkgs-minio {
                    inherit (final) system;
                    config.allowUnfree = true;
                  };
                })
              ];
            };

            overlayAttrs = {
              inherit (config.packages) rustToolchain;
            };

            packages.rustToolchain =
              with inputs.fenix.packages.${pkgs.stdenv.hostPlatform.system};
              combine [
                stable.clippy
                stable.rustc
                stable.cargo
                stable.rustfmt
                stable.rust-src
                # For Leptos
                targets.wasm32-unknown-unknown.stable.rust-std
              ];

            # `process-compose.foo` will add a flake package output called "foo".
            # Therefore, this will add a default package that you can build using
            # `nix build` and run using `nix run`.
            process-compose."default" =
              { config, ... }:
              let
                dbName = "db";
              in
              {
                imports = [
                  inputs.services-flake.processComposeModules.default
                ];

                services = {
                  minio."minio1" = {
                    enable = true;
                    package = pkgs.minio.minio;
                  };
                  postgres = {
                    "pg1" = {
                      enable = true;
                      extensions = extensions: [
                        extensions.postgis
                      ];
                      initialDatabases = [
                        {
                          name = dbName;
                        }
                      ];
                    };
                  };
                };

                settings = {
                  processes = {
                    minio1-test = {
                      command = pkgs.writeShellApplication {
                        runtimeInputs = [ pkgs.curl ];
                        text = ''
                          curl http://127.0.0.1:9000/minio/health/live
                        '';
                        name = "minio1-test";
                      };
                      depends_on."minio1".condition = "process_healthy";
                    };
                    pgweb =
                      let
                        pgcfg = config.services.postgres.pg1;
                      in
                      {
                        environment.PGWEB_DATABASE_URL = pgcfg.connectionURI { inherit dbName; };
                        command = pkgs.pgweb;
                        depends_on."pg1".condition = "process_healthy";
                      };
                    pg1-test = {
                      command = pkgs.writeShellApplication {
                        name = "pg1-test";
                        runtimeInputs = [ config.services.postgres.pg1.package ];
                        text = ''
                          echo 'SELECT version();' | psql -h 127.0.0.1 ${dbName}
                        '';
                      };
                      depends_on."pg1".condition = "process_healthy";
                    };
                  };
                };
              };

            devShells.default = pkgs.mkShell {
              inputsFrom = [
                config.process-compose."default".services.outputs.devShell
              ];

              nativeBuildInputs = [ pkgs.just ];

              packages = [
                pkgs.bashInteractive
                config.packages.rustToolchain
                pkgs.openssl
                pkgs.pkg-config
                pkgs.cargo-deny
                pkgs.cargo-edit
                pkgs.bacon
                pkgs.rust-analyzer
                # For Leptos
                pkgs.trunk
              ];

              env = {
                # Required by rust-analyzer
                RUST_SRC_PATH = "${config.packages.rustToolchain}/lib/rustlib/src/rust/library";
                # Required by minio-rs dependency
                LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.openssl ];
              };
            };
          };
      }
    );
}
