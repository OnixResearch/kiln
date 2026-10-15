{
  description = "Kiln provider-neutral CI control plane";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-eval-jobs.url = "github:NixOS/nix-eval-jobs/a0cd02231c58974a6b5aaa3712069b071047162e";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      nix-eval-jobs,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      eachSystem = function: nixpkgs.lib.genAttrs systems (system: function system);
      cairnGateSource = ./vendor/cairn;
      cairnGateRevision = (builtins.fromJSON (builtins.readFile ./vendor/cairn.source.json)).source_commit;
      mantleBuildSource = ./vendor/mantle;
      mantleBuildRevision = (builtins.fromJSON (builtins.readFile ./vendor/mantle.source.json)).source_commit;
      nixEvalJobsPackageFor = system: nix-eval-jobs.packages.${system}.nix-eval-jobs;
      celldRelease = "v0.2.1";
      celldRevision = "ae8fac053d79f971bfcb996054bb43eb2f9b05da";
      celldSourceHash = "sha256-DkudpH4V3EASbApSUadElsOINit+UfV+RQ+oHkOiylU=";
      celldCohortPackageFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        pkgs.stdenv.mkDerivation {
          pname = "celld";
          version = celldRelease;
          src = pkgs.fetchurl {
            name = "celld-${celldRelease}.tar.gz";
            url = "https://github.com/denoland/celld/archive/refs/tags/${celldRelease}.tar.gz";
            sha256 = celldSourceHash;
          };
          phases = [ "unpackPhase" "installPhase" ];
          installPhase = ''
            mkdir -p "$out"
            b3sum "$src" > "$out/celld-source.blake3"
            echo "${celldRevision}" > "$out/rev"
          '';
          nativeBuildInputs = [ pkgs.b3sum ];
          meta.description = "Pinned celld source cohort for the Kiln event-store adapter";
        };
      secretspecCohortVersion = "0.19.0";
      secretspecPackageFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        pkgs.rustPlatform.buildRustPackage {
          pname = "secretspec";
          version = secretspecCohortVersion;
          src = pkgs.fetchCrate {
            pname = "secretspec";
            version = secretspecCohortVersion;
            hash = "sha256-tpzmzChyyYogebNZZi3LT61MO1HKZW8ln+21CwlqW8M=";
          };
          cargoHash = "sha256-VO05AAjBqNVowY2AsyF2W1k4sXWJxOw1U0krs13JS28=";
          doCheck = false;
        };
      pijulCohortVersion = "1.0.0-beta.11";
      pijulCohortSourceFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        pkgs.fetchurl {
          name = "pijul-${pijulCohortVersion}.tar.gz";
          url = "https://static.crates.io/crates/pijul/pijul-${pijulCohortVersion}.crate";
          hash = "sha256-ik/Ceqge4GExDVf84t+cxF8xSd2wC9+rK4Fr6wNZsT0=";
        };
      pijulCohortPackageFor =
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
          pname = "pijul";
          version = pijulCohortVersion;
          src = pijulCohortSourceFor system;
          cargoHash = "sha256-IhArTiReUdj49bA+XseQpOiszK801xX5LdLj8vXD8rs=";
          # zstd-seekable enumerates native sources in filesystem order.
          # Patch only the writable vendor copy; the registry input stays pinned.
          postPatch = ''
            patch --batch --fuzz=0 -p1 \
              -d "$cargoDepsCopy/source-registry-0/zstd-seekable-0.1.23" \
              < ${./patches/zstd-seekable-sort-sources.patch}
          '';
          buildNoDefaultFeatures = true;
          cargoBuildFlags = [
            "--bin"
            "pijul"
          ];
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [
            pkgs.dbus
            pkgs.libsodium
            pkgs.openssl
          ];
          doCheck = false;
        };
      nixEvalJobsCohortManifestFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          package = nixEvalJobsPackageFor system;
        in
        pkgs.writeText "kiln-nix-eval-jobs-cohort.json" (
          builtins.toJSON {
            schema = "kiln.nix-eval-jobs-cohort.v1";
            upstream_revision = "a0cd02231c58974a6b5aaa3712069b071047162e";
            adapter_version = "2.35.0";
            source_version = "0.1.6";
            nix_revision = "aa2613fa02ca6199fe0ebf61a0f7f6d781d32a47";
            nix_version = "2.35.2";
            nixpkgs_revision = "346dd96ad74dc4457a9db9de4f4f57dab2e5731d";
            platform = system;
            executable = "${package}/bin/nix-eval-jobs";
          }
        );
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
        celld-cohort = celldCohortPackageFor system;
        pijul-cohort-fixture-producer = pijulCohortPackageFor system;
        secretspec-cohort = secretspecPackageFor system;
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
          nixEvalWorkerCount = 1;
          nixEvalMemoryMebibytes = 512;
          nixEvalExpectedRecordCount = 3;
          nixEvalExpectedRootCount = 2;
          nixEvalOnixRecordCount = 1;
          pijulSupported = system == "x86_64-linux";
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
            # Check every member, not the pristine partial producer workspaces.
            cargo fmt -- --check
            touch "$out"
          '';
          clippy = rustPlatform.buildRustPackage {
            pname = "kiln-clippy";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
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
          pijul-native-fixtures =
            if pijulSupported then
              rustPlatform.buildRustPackage {
                pname = "kiln-pijul-native-fixtures";
                version = "0.1.0";
                src = self;
                cargoLock = {
                  lockFile = ./Cargo.lock;
                };
                nativeBuildInputs = [
                  pkgs.cmake
                  pkgs.pkg-config
                ];
                buildInputs = [ pkgs.zlib ];
                buildPhase = ''
                  runHook preBuild
                  export KILN_PIJUL_EXE=${pijulCohortPackageFor system}/bin/pijul
                  cargo test -p kiln-adapter-pijul --features native-fixtures --test native_repository --offline
                  runHook postBuild
                '';
                doCheck = false;
                installPhase = ''
                  touch "$out"
                '';
              }
            else
              pkgs.runCommand "kiln-pijul-native-fixtures-unsupported" { } ''
                touch "$out"
              '';
          pijul-cohort =
            if pijulSupported then
              pkgs.runCommand "kiln-pijul-cohort"
                {
                  nativeBuildInputs = [
                    pkgs.b3sum
                    pkgs.coreutils
                    pkgs.gnutar
                    pkgs.gzip
                  ];
                }
                ''
                  source=${pijulCohortSourceFor system}
                  executable=${pijulCohortPackageFor system}/bin/pijul
                  lock_context_lines=2
                  test "$(sha256sum "$source" | cut -d ' ' -f 1)" = \
                    '8a4fc27aa81ee061310d57fce2df9cc45f3149ddb00bdfab2b816beb0359b13d'
                  test "$executable" = \
                    '/nix/store/qr2sja5n9r9m0j9q2pyn60cyb6lg3vnp-pijul-1.0.0-beta.11/bin/pijul'
                  test "$(b3sum --no-names "$executable")" = \
                    '9b8d58a1ced03ceb73347e4ed4818d17cdbff2de66d9024d4ec9b87327a4f934'
                  test "$($executable --version)" = 'pijul 1.0.0-beta.11'
                  tar -xOf "$source" pijul-1.0.0-beta.11/Cargo.lock > producer-Cargo.lock
                  grep -A "$lock_context_lines" -F 'name = "libpijul"' producer-Cargo.lock | \
                    grep -F 'version = "1.0.0-beta.11"' >/dev/null
                  grep -A "$lock_context_lines" -F 'name = "pijul-repository"' producer-Cargo.lock | \
                    grep -F 'version = "1.0.0-beta.10"' >/dev/null
                  grep -A "$lock_context_lines" -F 'name = "sanakirja"' producer-Cargo.lock | \
                    grep -F 'version = "2.0.0-beta"' >/dev/null
                  b3sum --no-names "$executable" > "$out"
                ''
            else
              pkgs.runCommand "kiln-pijul-cohort-unsupported" { } ''
                touch "$out"
              '';
          nix-eval-fixture-process = rustPlatform.buildRustPackage {
            pname = "kiln-nix-eval-fixture-process";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            nativeBuildInputs = [
              pkgs.cmake
              pkgs.pkg-config
            ];
            buildInputs = [ pkgs.zlib ];
            buildPhase = ''
              runHook preBuild
              cargo test -p kiln-adapter-nix-eval --features fixture-process --test fixture_process --offline
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
            };
            cargoBuildFlags = [
              "-p"
              "kiln-core"
              "-p"
              "kiln-radicle-source-contract"
              "--target=wasm32-unknown-unknown"
            ];
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
          radicle-source-contract-fixtures = rustPlatform.buildRustPackage {
            pname = "kiln-radicle-source-contract-fixtures";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            nativeBuildInputs = [ pkgs.coreutils ];
            buildPhase = ''
              runHook preBuild
              cargo test -p kiln-radicle-source-contract --offline
              cargo run -q -p kiln-radicle-source-contract --example emit-fixtures --offline > generated.json
              cmp generated.json fixtures/radicle-source-contract/producer/fixtures-v1.json
              runHook postBuild
            '';
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
            nickel export config/pijul-source-profile.ncl >/dev/null
            nickel export config/lattice-workflow-profile.ncl >/dev/null
            nickel export config/cairn-gate-profile.ncl >/dev/null
            nickel export config/mantle-build-profile.ncl >/dev/null
            nickel export config/nix-eval-jobs-profile.ncl >/dev/null
            nickel export config/radicle-workflow-shell.ncl >/dev/null
            nickel export config/radicle-source-profile.ncl >/dev/null
            nickel export config/radicle-status-profile.ncl >/dev/null
            nickel export config/radicle-mutation-profile.ncl >/dev/null
            nickel export config/stack-profiles.ncl >/dev/null
            nickel export config/cdevents-profile.ncl >/dev/null
            nickel export config/opentelemetry-profile.ncl >/dev/null
            nickel export config/valence-profile.ncl >/dev/null
            nickel export config/webhook-profile.ncl >/dev/null
            nickel export config/sqlite-profile.ncl >/dev/null
            nickel export config/celld-profile.ncl >/dev/null
            nickel export config/celld-wrangler.ncl >/dev/null
            nickel export config/fixtures/celld-positive.ncl >/dev/null
            nickel export config/fixtures/git-source-positive.ncl >/dev/null
            nickel export config/fixtures/pijul-source-positive.ncl >/dev/null
            nickel export config/fixtures/lattice-workflow-positive.ncl >/dev/null
            nickel export config/fixtures/cairn-gate-positive.ncl >/dev/null
            nickel export config/fixtures/mantle-build-positive.ncl >/dev/null
            nickel export config/fixtures/nix-eval-jobs-positive.ncl >/dev/null
            nickel export config/fixtures/radicle-workflow-positive.ncl >/dev/null
            nickel export config/fixtures/radicle-source-positive.ncl >/dev/null
            nickel export config/fixtures/radicle-status-positive.ncl >/dev/null
            nickel export config/fixtures/radicle-mutation-positive.ncl >/dev/null
            nickel export config/fixtures/cairn-cob-external.ncl >/dev/null
            nickel export fixtures/radicle-source-contract/positive/receipt.ncl >/dev/null
            for fixture in fixtures/radicle-source-contract/negative/*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative Radicle source contract fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel export fixtures/lattice/workflow-contract/70496e67c7fd4a8b05914161a8e09de2759bebc8/source.ncl >/dev/null
            nickel export fixtures/cairn/gate-contract/15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/source.ncl >/dev/null
            nickel export fixtures/cairn/gate-machine/15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/source.ncl >/dev/null
            nickel export fixtures/mantle/build-contract/0bbab77e23ab95caa4e4184b9945cfaa5c8040e2/source.ncl >/dev/null
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
          cairn-gate-cohort = pkgs.runCommand "kiln-cairn-gate-cohort" { } ''
            test '${cairnGateRevision}' = '15f00875562025e7ea7e0d1f4af24d1a2e2ac06f'
            cmp ${cairnGateSource}/fixtures/cairn-gate-contract/producer/fixtures-v1.json \
              ${self}/fixtures/cairn/gate-contract/15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/producer-fixtures-v1.json
            cmp ${cairnGateSource}/contracts/cairn-gate-contract.ncl \
              ${self}/fixtures/cairn/gate-contract/15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/contract.ncl
            touch "$out"
          '';
          mantle-build-cohort =
            pkgs.runCommand "kiln-mantle-build-cohort" { nativeBuildInputs = [ pkgs.b3sum ]; }
              ''
                test '${mantleBuildRevision}' = '0bbab77e23ab95caa4e4184b9945cfaa5c8040e2'
                cmp ${mantleBuildSource}/fixtures/mantle-build-contract/producer/fixtures-v1.json \
                  ${self}/fixtures/mantle/build-contract/0bbab77e23ab95caa4e4184b9945cfaa5c8040e2/producer-fixtures-v1.json
                cmp ${mantleBuildSource}/contracts/mantle-build-contract.ncl \
                  ${self}/fixtures/mantle/build-contract/0bbab77e23ab95caa4e4184b9945cfaa5c8040e2/contract.ncl
                test "$(b3sum --no-names ${self}/config/fixtures/mantle-build-plan.ncl)" = \
                  '2a44f29408b31e620e9247134f0b7b8d55012c8ae286de29199eecef33e10473'
                test "$(b3sum --no-names ${self}/config/fixtures/mantle-build-policy.ncl)" = \
                  '6a094f32254723850459379582ef03c8d2dad65c25326ca526ba3fe62aa9d726'
                test "$(b3sum --no-names ${self}/config/fixtures/mantle-build-worker.ncl)" = \
                  '6f66ef1d446ead1d87f37176335a3876e6c98af108febf4ec98600b3a91a8d4e'
                test "$(b3sum --no-names ${self}/config/fixtures/mantle-store-identity.json)" = \
                  '4bd89c0869c39e8d17e5a6799f977bdac78bbc72c93d7b97cb8348b5d3f69b57'
                touch "$out"
              '';
          nix-eval-jobs-cohort =
            if system == "x86_64-linux" then
              pkgs.runCommand "kiln-nix-eval-jobs-cohort"
                {
                  nativeBuildInputs = [
                    (nixEvalJobsPackageFor system)
                    pkgs.b3sum
                    pkgs.coreutils
                    pkgs.gnugrep
                    pkgs.nix
                  ];
                }
                ''
                  test '${nix-eval-jobs.rev}' = 'a0cd02231c58974a6b5aaa3712069b071047162e'
                  executable=${nixEvalJobsPackageFor system}/bin/nix-eval-jobs
                  manifest=${nixEvalJobsCohortManifestFor system}
                  test "$executable" = \
                    '/nix/store/bcv7hkwwpmfjmvxh6i106gdwr956svsp-nix-eval-jobs-2.35.0/bin/nix-eval-jobs'
                  test "$manifest" = \
                    '/nix/store/wasviibxqrxh6h8wckaifm2s6lancgpn-kiln-nix-eval-jobs-cohort.json'
                  test "$(b3sum --no-names "$executable")" = \
                    'dc42bc0e330c598ec93c2361afbb493417fa5c3364722897bd156d3f2ba9c063'
                  test "$(b3sum --no-names "$manifest")" = \
                    '102b4af9a753d00dfb6489e1593dd4b9ac8ffe9eed42518705329c6018e63c31'
                  grep -F '"nix_version":"2.35.2"' "$manifest" >/dev/null
                  grep -F '"nix_revision":"aa2613fa02ca6199fe0ebf61a0f7f6d781d32a47"' "$manifest" >/dev/null

                  export HOME=$TMPDIR/home
                  export NIX_STATE_DIR=$TMPDIR/nix-state
                  export NIX_STORE_DIR=$TMPDIR/nix-store
                  export NIX_DATA_DIR=$TMPDIR/nix-data
                  export NIX_LOG_DIR=$TMPDIR/nix-log
                  export NIX_CONF_DIR=$TMPDIR/nix-conf
                  mkdir -p "$HOME" "$NIX_STATE_DIR" "$NIX_STORE_DIR" "$NIX_DATA_DIR" "$NIX_LOG_DIR" "$NIX_CONF_DIR" roots roots-onix
                  cp -r ${self}/fixtures/nix-eval-jobs/cohort/a0cd02231c58974a6b5aaa3712069b071047162e/jobset jobset
                  chmod -R u+w jobset
                  cat > "$NIX_CONF_DIR/nix.conf" <<'EOF'
                  experimental-features = nix-command flakes
                  accept-flake-config = false
                  flake-registry =
                  substituters =
                  sandbox = false
                  EOF

                  "$executable" \
                    --gc-roots-dir roots \
                    --workers ${toString nixEvalWorkerCount} \
                    --max-memory-size ${toString nixEvalMemoryMebibytes} \
                    --extra-experimental-features 'nix-command flakes' \
                    --meta \
                    --show-input-drvs \
                    --check-cache-status \
                    --flake "$PWD/jobset#jobs" \
                    > producer.jsonl
                  test "$(wc -l < producer.jsonl)" -eq ${toString nixEvalExpectedRecordCount}
                  test "$(find roots -mindepth 1 -maxdepth 1 | wc -l)" -eq ${toString nixEvalExpectedRootCount}
                  grep -F '"attr":"job-a"' producer.jsonl >/dev/null
                  grep -F '"attr":"job-b"' producer.jsonl >/dev/null
                  grep -F '"attr":"broken"' producer.jsonl >/dev/null
                  grep -F '"cacheStatus":"notBuilt"' producer.jsonl >/dev/null
                  grep -F '"inputDrvs"' producer.jsonl >/dev/null

                  "$executable" \
                    --gc-roots-dir roots-onix \
                    --workers ${toString nixEvalWorkerCount} \
                    --max-memory-size ${toString nixEvalMemoryMebibytes} \
                    --extra-experimental-features 'nix-command flakes' \
                    --meta \
                    --show-input-drvs \
                    --check-cache-status \
                    --flake "$PWD/jobset#onix-only" \
                    > onix.jsonl
                  test "$(wc -l < onix.jsonl)" -eq ${toString nixEvalOnixRecordCount}
                  grep -F '"attr":"unsupported"' onix.jsonl >/dev/null
                  grep -F "attribute 'wasm' missing" onix.jsonl >/dev/null
                  touch "$out"
                ''
            else
              pkgs.runCommand "kiln-nix-eval-jobs-cohort-unsupported" { } ''
                touch "$out"
              '';
          radicle-source-native-fixtures =
            if system == "x86_64-linux" then
              rustPlatform.buildRustPackage {
                pname = "kiln-radicle-source-native-fixtures";
                version = "0.1.0";
                src = self;
                cargoLock = {
                  lockFile = ./Cargo.lock;
                };
                nativeBuildInputs = [
                  pkgs.cmake
                  pkgs.pkg-config
                ];
                buildInputs = [ pkgs.zlib ];
                buildPhase = ''
                  runHook preBuild
                  cargo test -p kiln-adapter-radicle --features source-native-fixtures --test source_native --offline
                  runHook postBuild
                '';
                doCheck = false;
                installPhase = ''
                  touch "$out"
                '';
              }
            else
              pkgs.runCommand "kiln-radicle-source-native-fixtures-unsupported" { } ''
                touch "$out"
              '';
          radicle-status-native-fixtures =
            if system == "x86_64-linux" then
              rustPlatform.buildRustPackage {
                pname = "kiln-radicle-status-native-fixtures";
                version = "0.1.0";
                src = self;
                cargoLock = {
                  lockFile = ./Cargo.lock;
                };
                nativeBuildInputs = [
                  pkgs.cmake
                  pkgs.pkg-config
                ];
                buildInputs = [ pkgs.zlib ];
                buildPhase = ''
                  runHook preBuild
                  cargo test -p kiln-adapter-radicle --features status-native-fixtures --test status_native --offline
                  runHook postBuild
                '';
                doCheck = false;
                installPhase = ''
                  touch "$out"
                '';
              }
            else
              pkgs.runCommand "kiln-radicle-status-native-fixtures-unsupported" { } ''
                touch "$out"
              '';
          radicle-mutation-native-fixtures =
            if system == "x86_64-linux" then
              rustPlatform.buildRustPackage {
                pname = "kiln-radicle-mutation-native-fixtures";
                version = "0.1.0";
                src = self;
                cargoLock = {
                  lockFile = ./Cargo.lock;
                };
                nativeBuildInputs = [
                  pkgs.cmake
                  pkgs.pkg-config
                ];
                buildInputs = [ pkgs.zlib ];
                buildPhase = ''
                  runHook preBuild
                  cargo test -p kiln-adapter-radicle --features status-native-fixtures --test mutation_native --offline
                  runHook postBuild
                '';
                doCheck = false;
                installPhase = ''
                  touch "$out"
                '';
              }
            else
              pkgs.runCommand "kiln-radicle-mutation-native-fixtures-unsupported" { } ''
                touch "$out"
              '';
          radicle-status-cohort =
            if system == "x86_64-linux" then
              pkgs.runCommand "kiln-radicle-status-cohort"
                {
                  nativeBuildInputs = [
                    (secretspecPackageFor system)
                    pkgs.b3sum
                    pkgs.gnugrep
                  ];
                }
                ''
                  executable=${secretspecPackageFor system}/bin/secretspec
                  lock_context_lines=3
                  test "$executable" = \
                    '/nix/store/a9hlwn1m4p0pi17yif3aa3ak2d0majz5-secretspec-0.19.0/bin/secretspec'
                  test "$($executable --version)" = 'secretspec 0.19.0'
                  test "$(b3sum --no-names "$executable")" = \
                    'c15ea6408e2ff98fd976507a2ae5bec3cb99ef5580ce7342ba92eb465d9b28a6'
                  grep -A "$lock_context_lines" -F 'name = "radicle-cob"' ${self}/Cargo.lock | \
                    grep -F 'version = "0.21.1"' >/dev/null
                  grep -A "$lock_context_lines" -F 'name = "radicle-core"' ${self}/Cargo.lock | \
                    grep -F 'version = "0.4.1"' >/dev/null
                  grep -F 'radicle-core = { version = "=0.4.1", default-features = false, features = ["std", "git2", "serde"] }' ${self}/Cargo.toml >/dev/null
                  grep -F '0a5924dd2092243a0fe5df94d43008f0402e46f6' ${self}/config/radicle-source-profile.ncl >/dev/null
                  grep -A "$lock_context_lines" -F 'name = "radicle-crypto"' ${self}/Cargo.lock | \
                    grep -F 'version = "0.19.0"' >/dev/null
                  grep -A "$lock_context_lines" -F 'name = "radicle-oid"' ${self}/Cargo.lock | \
                    grep -F 'version = "0.2.2"' >/dev/null
                  grep -A "$lock_context_lines" -F 'name = "radicle-git-ref-format"' ${self}/Cargo.lock | \
                    grep -F 'version = "0.2.0"' >/dev/null
                  grep -A "$lock_context_lines" -F 'name = "git2"' ${self}/Cargo.lock | \
                    grep -F 'version = "0.21.0"' >/dev/null
                  if grep -F 'name = "radicle"' ${self}/Cargo.lock >/dev/null; then
                    echo 'full Radicle runtime entered the status cohort' >&2
                    exit 1
                  fi
                  touch "$out"
                ''
            else
              pkgs.runCommand "kiln-radicle-status-cohort-unsupported" { } ''
                touch "$out"
              '';
          radicle-status-boundaries = rustPlatform.buildRustPackage {
            pname = "kiln-radicle-status-boundaries";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            nativeBuildInputs = [
              pkgs.cmake
              pkgs.pkg-config
            ];
            buildInputs = [ pkgs.zlib ];
            buildPhase = ''
              runHook preBuild
              cargo tree -p kiln-core --edges normal > core-tree.txt
              cargo tree -p kiln-standalone --edges normal > standalone-tree.txt
              cargo tree -p kiln-radicle-source-contract --edges normal | tail -n +2 > source-contract-tree.txt
              if grep -E 'radicle|secretspec' core-tree.txt standalone-tree.txt >/dev/null; then
                echo 'Radicle or SecretSpec entered a runtime-independent crate' >&2
                exit 1
              fi
              if grep -E 'git2|rusqlite|bounded-exec|radicle-(cob|core|crypto|git|oid)|kiln-(core|provider-api|codec)' source-contract-tree.txt >/dev/null; then
                echo 'Provider runtime dependency entered the Radicle source contract' >&2
                exit 1
              fi
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
          celld-boundaries = rustPlatform.buildRustPackage {
            pname = "kiln-celld-boundaries";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            buildPhase = ''
              runHook preBuild
              sh tests/celld-boundaries.sh
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
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
