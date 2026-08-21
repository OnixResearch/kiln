{
  description = "Kiln provider-neutral CI control plane";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      eachSystem = function: nixpkgs.lib.genAttrs systems (system: function system);
      packageFor =
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
        in
        rustPlatform.buildRustPackage {
          pname = "kiln";
          version = "0.1.0";
          src = self;
          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };
          nativeBuildInputs = [
            pkgs.cmake
            pkgs.pkg-config
          ];
          buildInputs = [ pkgs.zlib ];
          cargoBuildFlags = [
            "--workspace"
            "--all-targets"
          ];
          cargoTestFlags = [ "--workspace" ];
        };
    in
    {
      packages = eachSystem (system: {
        kiln = packageFor system;
        default = packageFor system;
      });

      apps = eachSystem (system: {
        kiln = {
          type = "app";
          program = "${packageFor system}/bin/kiln";
          meta.description = "Run the Kiln local operator CLI";
        };
        default = self.apps.${system}.kiln;
      });

      checks = eachSystem (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
          package = packageFor system;
          copySource = ''
            cp -r ${self} source
            chmod -R u+w source
            cd source
          '';
        in
        {
          inherit package;
          tests = package;
          boundaries = package;
          runtime-independence = package;
          format = pkgs.runCommand "kiln-format" { nativeBuildInputs = [ toolchain ]; } ''
            ${copySource}
            cargo fmt --all -- --check
            touch "$out"
          '';
          clippy = rustPlatform.buildRustPackage {
            pname = "kiln-clippy";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
              allowBuiltinFetchGit = true;
            };
            nativeBuildInputs = [
              pkgs.clippy
              pkgs.cmake
              pkgs.pkg-config
            ];
            buildInputs = [ pkgs.zlib ];
            buildPhase = ''
              runHook preBuild
              cargo clippy --workspace --all-targets -- -D warnings
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
          no-std = rustPlatform.buildRustPackage {
            pname = "kiln-no-std";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
              allowBuiltinFetchGit = true;
            };
            cargoBuildFlags = [
              "-p"
              "kiln-core"
              "--target=wasm32-unknown-unknown"
            ];
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
          nickel = pkgs.runCommand "kiln-nickel" { nativeBuildInputs = [ pkgs.nickel ]; } ''
            ${copySource}
            nickel export config/default.ncl >/dev/null
            nickel export config/hosting-profile.ncl >/dev/null
            nickel export config/cob-profiles.ncl >/dev/null
            nickel export config/git-source-profile.ncl >/dev/null
            nickel export config/lattice-workflow-profile.ncl >/dev/null
            nickel export config/radicle-workflow-shell.ncl >/dev/null
            nickel export config/stack-profiles.ncl >/dev/null
            nickel export config/cdevents-profile.ncl >/dev/null
            nickel export config/opentelemetry-profile.ncl >/dev/null
            nickel export config/valence-profile.ncl >/dev/null
            nickel export config/webhook-profile.ncl >/dev/null
            nickel export config/sqlite-profile.ncl >/dev/null
            nickel export config/fixtures/positive.ncl >/dev/null
            nickel export config/fixtures/git-source-positive.ncl >/dev/null
            nickel export config/fixtures/lattice-workflow-positive.ncl >/dev/null
            nickel export config/fixtures/radicle-workflow-positive.ncl >/dev/null
            nickel export config/fixtures/cairn-cob-external.ncl >/dev/null
            nickel export fixtures/lattice/workflow-contract/70496e67c7fd4a8b05914161a8e09de2759bebc8/source.ncl >/dev/null
            nickel export evidence/publication-candidate.ncl >/dev/null
            nickel export evidence/publication-receipt.ncl >/dev/null
            nickel export evidence/release-input.ncl >/dev/null
            for fixture in config/fixtures/negative-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative Nickel fixture passed: $fixture" >&2
                exit 1
              fi
            done
            touch "$out"
          '';
          release-evidence =
            pkgs.runCommand "kiln-release-evidence"
              {
                nativeBuildInputs = [
                  package
                  pkgs.nickel
                ];
              }
              ''
                ${copySource}
                nickel export evidence/release-input.ncl --format json > release-input.json
                kiln evidence release-input.json release-evidence.json >/dev/null
                cmp release-evidence.json evidence/release-evidence.json
                touch "$out"
              '';
        }
      );

      devShells = eachSystem (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        in
        {
          default = pkgs.mkShell {
            packages = [
              toolchain
              pkgs.cmake
              pkgs.nickel
              pkgs.nixfmt
              pkgs.pkg-config
              pkgs.zlib
            ];
          };
        }
      );
    };
}
