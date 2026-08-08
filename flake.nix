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
          cargoLock.lockFile = ./Cargo.lock;
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
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.clippy ];
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
            cargoLock.lockFile = ./Cargo.lock;
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
            nickel export config/stack-profiles.ncl >/dev/null
            nickel export config/fixtures/positive.ncl >/dev/null
            nickel export config/fixtures/cairn-cob-external.ncl >/dev/null
            for fixture in config/fixtures/negative-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative Nickel fixture passed: $fixture" >&2
                exit 1
              fi
            done
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
              pkgs.nickel
              pkgs.nixfmt
            ];
          };
        }
      );
    };
}
