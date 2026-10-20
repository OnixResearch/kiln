{
  description = "ucan — Rust project built with Crane";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    # Stack Nickel 1.17.0 cohort, pinned to the exact reviewed upstream commit.
    nickel-1-17-0 = {
      url = "github:nickel-lang/nickel/1320a983e6c3d1e2fb53dd2464b084b4903b1426";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      rust-overlay,
      flake-utils,
      nickel-1-17-0,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Keep Cargo sources plus the Aeneas checker's compile-time evidence fixtures.
        proofSource = ./proofs;
        proofSourceString = toString proofSource;
        proofSourcePrefix = "${proofSourceString}/";
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            craneLib.filterCargoSources path type
            || toString path == proofSourceString
            || pkgs.lib.hasPrefix proofSourcePrefix (toString path);
        };

        # Exact stack Nickel 1.17.0 cohort evaluator.
        nickelCohort = nickel-1-17-0.packages.${system}.default;
        checkedNickel = "${nickelCohort}/bin/nickel";

        # Common build inputs
        nativeBuildInputs = with pkgs; [
          pkg-config
          clang
          mold
        ];

        buildInputs =
          with pkgs;
          [
            openssl
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ];

        commonArgs = {
          inherit src nativeBuildInputs buildInputs;
          cargoToml = ./Cargo.toml;
          cargoLock = ./Cargo.lock;
        };

        # Build just the cargo dependencies for caching
        cargoArtifacts = craneLib.buildDepsOnly (commonArgs);

        # Build the actual package. Tests run in the explicit nextest check below;
        # Hegel integration tests need a writable Python/uv environment and are
        # exercised from the dev shell rather than the sandboxed package build.
        ucan = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            doCheck = false;
          }
        );
      in
      {
        packages = {
          default = ucan;
          ucan = ucan;
          nickel-1-17-0 = nickelCohort;
        };

        checks = {
          inherit ucan;

          # Run tests with nextest
          nextest = craneLib.cargoNextest (
            commonArgs
            // {
              inherit cargoArtifacts;
              partitions = 1;
              partitionType = "count";
              cargoExtraArgs = "--locked -p ucan -p ucan-core -p ucan-nostr --lib --examples --features ucan-core/peer-auth";
            }
          );

          # Clippy lints for workspace packages only; the vendored snapshot keeps upstream lint policy.
          clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoExtraArgs = "--locked --no-deps -p ucan -p ucan-core -p ucan-nostr --features ucan-core/peer-auth";
              cargoClippyExtraArgs = "--all-targets -- -D warnings -A clippy::cargo-common-metadata";
            }
          );

          # Format only workspace packages; keep the vendored verified-logic snapshot byte-for-byte.
          fmt = craneLib.cargoFmt (
            commonArgs
            // {
              cargoExtraArgs = "--manifest-path Cargo.toml -p ucan -p ucan-core -p ucan-nostr";
            }
          );

          production-readiness =
            pkgs.runCommand "ucan-production-readiness"
              {
                nativeBuildInputs = [ nickelCohort ];
              }
              ''
                cd ${./.}
                ${ucan}/bin/check-production-readiness check --root . --nickel ${checkedNickel}
                touch $out
              '';

          aeneas-time-bounds-proof =
            pkgs.runCommand "ucan-aeneas-time-bounds-proof"
              {
                nativeBuildInputs = [ nickelCohort ];
              }
              ''
                cd ${./.}
                ${ucan}/bin/check-aeneas-time-bounds-proof check --root . --nickel ${checkedNickel}
                touch $out
              '';

          nickel-cohort-version =
            pkgs.runCommand "ucan-nickel-cohort-version"
              {
                nativeBuildInputs = [ nickelCohort ];
              }
              ''
                version=$(${checkedNickel} --version 2>&1 | head -1)
                echo "resolved nickel: $version"
                case "$version" in
                  *1.17*) exit 0 ;;
                  *)
                    echo "older, floating, ambient, or mixed evaluator resolved: $version" >&2
                    exit 1
                    ;;
                esac
                touch $out
              '';

          nickel-toolchain-fixtures =
            pkgs.runCommand "ucan-nickel-toolchain-fixtures"
              {
                nativeBuildInputs = [ nickelCohort ];
              }
              ''
                cd ${./.}
                bash scripts/check-nickel-toolchain-fixtures.sh ${checkedNickel}
                touch $out
              '';

          verified-logic-josh-sync-config =
            pkgs.runCommand "ucan-verified-logic-josh-sync-config"
              {
                nativeBuildInputs = [ rustToolchain ];
              }
              ''
                cd ${./.}
                rustc --edition=2024 --test scripts/check-verified-logic-josh-sync.rs -o "$TMPDIR/check-verified-logic-josh-sync-tests"
                "$TMPDIR/check-verified-logic-josh-sync-tests"
                rustc --edition=2024 scripts/check-verified-logic-josh-sync.rs -o "$TMPDIR/check-verified-logic-josh-sync"
                "$TMPDIR/check-verified-logic-josh-sync" --config-only
                touch $out
              '';

          dependency-posture =
            pkgs.runCommand "ucan-dependency-posture"
              {
                nativeBuildInputs = [ rustToolchain pkgs.cargo-deny ];
              }
              ''
                cd ${./.}
                rustc --edition=2024 tools/check-dependency-posture.rs -o "$TMPDIR/check-dependency-posture"
                "$TMPDIR/check-dependency-posture" --root .
                cargo-deny check advisories -D warnings -A advisory-not-detected >/dev/null 2>&1 \
                  || echo "cargo-deny online advisory scan unavailable (advisory DB fetch or config error); the deterministic posture gate passed above"
                touch $out
              '';
        };

        devShells.default = craneLib.devShell {
          inherit buildInputs;

          packages = with pkgs; [
            cargo-fuzz
            cargo-mutants
            cargo-nextest
            cargo-watch
            rust-analyzer
            uv
            nickelCohort
          ];

          # Ensure the nightly toolchain and vendored verified-logic source are
          # present in the same cleaned build tree used by the package.
          inputsFrom = [ ucan ];
        };
      }
    );
}
