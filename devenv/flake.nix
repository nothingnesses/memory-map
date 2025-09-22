{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.1";
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
            ...
          }:
          {
            # Recommended: move all package definitions here.
            # e.g. (assuming you have a nixpkgs input)
            # packages.foo = pkgs.callPackage ./foo/package.nix { };
            # packages.bar = pkgs.callPackage ./bar/package.nix {
            #   foo = config.packages.foo;
            # };

            overlayAttrs = {
              inherit (config.packages) rustToolchain;
            };

            packages.rustToolchain =
              with inputs.fenix.packages.${pkgs.stdenv.hostPlatform.system};
              combine (
                with stable;
                [
                  clippy
                  rustc
                  cargo
                  rustfmt
                  rust-src
                ]
              );

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

                settings.processes.pgweb =
                  let
                    pgcfg = config.services.postgres.pg1;
                  in
                  {
                    environment.PGWEB_DATABASE_URL = pgcfg.connectionURI { inherit dbName; };
                    command = pkgs.pgweb;
                    depends_on."pg1".condition = "process_healthy";
                  };
                settings.processes.test = {
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
                pkgs.cargo-watch
                pkgs.rust-analyzer
              ];

              env = {
                # Required by rust-analyzer
                RUST_SRC_PATH = "${config.packages.rustToolchain}/lib/rustlib/src/rust/library";
              };
            };
          };
      }
    );
}
