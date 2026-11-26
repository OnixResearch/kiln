{
  description = "Kiln provider-neutral CI control plane";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # Guest-only cohort: do not move the separately qualified build producers.
    guest-nixpkgs.url = "github:NixOS/nixpkgs/1d060086d60cb76a40cf37109f979d88a5bd6396";
    unikraft = {
      url = "github:unikraft/unikraft/a4c494e80141bd402961c7d96150dd218c79db37";
      flake = false;
    };
    app-elfloader = {
      url = "github:unikraft/app-elfloader/0cd7a2b2c4d5ece85647c0c9903b50426e849420";
      flake = false;
    };
    lib-libelf = {
      url = "github:unikraft/lib-libelf/c03d4ceb22a2b9e9799238f9c277a14284d313ad";
      flake = false;
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-eval-jobs.url = "github:NixOS/nix-eval-jobs/a0cd02231c58974a6b5aaa3712069b071047162e";
    cairn = {
      url = "github:OnixResearch/cairn/317d3ff648f01a5ea1572e6126f4dd71bc7afa99";
    };
    bombadil = {
      url = "github:antithesishq/bombadil/832faf4d953e2c56d1d901aca1bb2c3c3170221d";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      guest-nixpkgs,
      unikraft,
      app-elfloader,
      lib-libelf,
      rust-overlay,
      nix-eval-jobs,
      cairn,
      bombadil,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      eachSystem = function: nixpkgs.lib.genAttrs systems (system: function system);
      # Keep private tool/runtime acquisition outside normal root builds. The
      # content-addressed subflake retains the canonical lock and nixpkgs follow.
      runtimeBuildCohort = builtins.getFlake (
        builtins.unsafeDiscardStringContext "path:${./runtime/build-cohort}?narHash=sha256-PcsYctZPpAGWzSNCAqmog3Dvqk7hePUu0TYwvbgpcu8="
      );
      octetInput =
        assert runtimeBuildCohort.inputs.nixpkgs.rev == nixpkgs.rev;
        runtimeBuildCohort.inputs.octet;
      # Lattice worker guest only: the exact Lattice producer package and its
      # own lock, loaded lazily from a content-addressed subflake.
      latticeWorkerCohort = builtins.getFlake (
        builtins.unsafeDiscardStringContext "path:${./runtime/lattice-worker-cohort}?narHash=sha256-L7FUng/7LiODFsWSLtQzylEI9+AS0ozOClNPJeqx+14="
      );
      kamacite-build-support = runtimeBuildCohort.inputs.kamacite-build-support;
      cairnGateSource = ./vendor/cairn;
      cairnGateRevision =
        (builtins.fromJSON (builtins.readFile ./vendor/cairn.source.json)).source_commit;
      cairnPackageFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          cairnPkg = cairn.packages.${system}.cairn;
          kilnPolicy = ./cairn-policy/generated/cairn-policy.json;
        in
        pkgs.writeShellApplication {
          name = "cairn";
          text = ''
            # Default to Kiln's generated policy regardless of any outer
            # CAIRN_POLICY (for example lattice-default). Prefer the live
            # checkout's policy when invoked inside a Kiln working tree;
            # explicit --policy flags always override CAIRN_POLICY in cairn.
            root=""
            prev=""
            for arg in "$@"; do
              if [ "$prev" = "--root" ]; then
                root="$arg"
                break
              fi
              case "$arg" in
                --root=*)
                  root="''${arg#--root=}"
                  break
                  ;;
              esac
              prev="$arg"
            done
            if [ -n "$root" ] && [ -f "$root/cairn-policy/generated/cairn-policy.json" ]; then
              export CAIRN_POLICY="$root/cairn-policy/generated/cairn-policy.json"
            elif [ -f "$PWD/cairn-policy/generated/cairn-policy.json" ]; then
              export CAIRN_POLICY="$PWD/cairn-policy/generated/cairn-policy.json"
            else
              export CAIRN_POLICY="${kilnPolicy}"
            fi
            exec "${cairnPkg}/bin/cairn" "$@"
          '';
        };
      mantleBuildSource = ./vendor/mantle;
      mantleBuildRevision =
        (builtins.fromJSON (builtins.readFile ./vendor/mantle.source.json)).source_commit;
      nixEvalJobsPackageFor = system: nix-eval-jobs.packages.${system}.nix-eval-jobs;
      unikernelFor =
        system:
        import ./runtime/unikernel {
          pkgs = import guest-nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          inherit unikraft app-elfloader lib-libelf;
        };
      prepareKamaciteBuildSupport = ''
        mkdir -p "$NIX_BUILD_TOP/adapters/spacewasm-mvp/generated"
        cp \
          ${kamacite-build-support}/adapters/spacewasm-mvp/generated/profile.json \
          "$NIX_BUILD_TOP/adapters/spacewasm-mvp/generated/profile.json"
      '';
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
          phases = [
            "unpackPhase"
            "installPhase"
          ];
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
      # Property-based testing rail for terminal surfaces. The binary is a
      # separately executed tool; it never links into Kiln.
      bombadilPackageFor = system: bombadil.packages.${system}.default;
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
          # The workspace tests include kiln-fleet's dispatch tests, which
          # create and clone fixture Git repositories.
          nativeCheckInputs = [
            pkgs.python3
            pkgs.git
          ];
          buildInputs = [
            pkgs.zlib
            pkgs.openssl
          ];
          cargoBuildFlags = [
            "--workspace"
            "--all-targets"
          ];
          cargoTestFlags = [ "--workspace" ];
          postInstall = ''
            install -Dm644 config/implementation-campaign-drive.ncl \
              "$out/share/kiln/implementation-campaign-drive.ncl"
          '';
        };
      # r[impl kiln.web.deploy.install]
      webServicePackageFor =
        system:
        (packageFor system).overrideAttrs (old: {
          pname = "kiln-web-service";
          cargoBuildFlags = [
            "-p"
            "kiln-web-service"
            "--bin"
            "kiln-web-service"
          ];
          cargoTestFlags = [
            "-p"
            "kiln-web-service"
          ];
          postInstall = (old.postInstall or "") + ''
            install -Dm644 deployment/kiln-web-service.service.in \
              "$out/lib/systemd/system/kiln-web-service.service"
            substituteInPlace "$out/lib/systemd/system/kiln-web-service.service" \
              --replace-fail '@out@' "$out"
          '';
          meta.description = "Authenticated single-tenant Kiln web service with supervised deployment assets";
        });
      # r[impl kiln.implementation_qualification.real_stages]
      implementationFixturesPackageFor =
        system:
        (packageFor system).overrideAttrs (_: {
          pname = "kiln-implementation-fixtures";
          buildFeatures = [
            "kiln-adapter-github/protocol-fixtures"
            "kiln-web-service/implementation-fixtures"
          ];
          cargoBuildFlags = [
            "-p"
            "kiln-adapter-github"
            "-p"
            "kiln-adapter-lattice"
            "-p"
            "kiln-adapter-cairn"
            "-p"
            "kiln-web-service"
            "--bins"
          ];
          cargoTestFlags = [
            "-p"
            "kiln-adapter-github"
            "-p"
            "kiln-adapter-lattice"
            "-p"
            "kiln-adapter-cairn"
            "-p"
            "kiln-web-service"
          ];
          meta.description = "Explicit local scripted implementation fixtures; no production provider fallback";
        });
      # r[impl kiln.web.exploration.scenario.deploy]
      websiteExplorationPackageFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        (packageFor system).overrideAttrs (_: {
          pname = "kiln-website-exploration";
          cargoBuildFlags = [
            "-p"
            "kiln-web-service"
            "--example"
            "explore"
          ];
          cargoTestFlags = [
            "-p"
            "kiln-web-service"
            "--example"
            "explore"
          ];
          # The base postInstall ships the web service's implementation-campaign
          # drive profile; the synthetic explorer has no use for it, and two
          # profile packages cannot provide the same file.
          postInstall = "";
          installPhase = ''
            runHook preInstall
            install -Dm755 target/${pkgs.stdenv.hostPlatform.rust.rustcTarget}/release/examples/explore \
              "$out/bin/kiln-website-exploration"
            install -Dm644 deployment/kiln-website-exploration.service.in \
              "$out/share/systemd/user/kiln-website-exploration.service"
            substituteInPlace "$out/share/systemd/user/kiln-website-exploration.service" \
              --replace-fail '@out@' "$out"
            runHook postInstall
          '';
          meta.description = "Disposable loopback-only Kiln website exploration with synthetic examples";
        });
      # r[impl kiln.web.exploration.browser_property_rail.scenario.cohort]
      webExplorationBombadilPackageFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        pkgs.writeShellApplication {
          name = "kiln-web-exploration-bombadil";
          text = ''
            exec ${pkgs.python3}/bin/python3 ${./scripts/web-exploration-bombadil.py} \
              --bombadil ${bombadilPackageFor system}/bin/bombadil \
              --program ${websiteExplorationPackageFor system}/bin/kiln-website-exploration \
              --browser ${pkgs.chromium}/bin/chromium \
              --specification ${./checks/bombadil/kiln-web-exploration.spec.ts} \
              "$@"
          '';
        };
      # Standalone Cairn intake binary from the adapter package.
      kilnCairnIntakePackageFor =
        system:
        (packageFor system).overrideAttrs (_: {
          pname = "kiln-cairn-intake";
          cargoBuildFlags = [
            "-p"
            "kiln-adapter-cairn"
            "--bin"
            "kiln-cairn-intake"
          ];
          cargoTestFlags = [
            "-p"
            "kiln-adapter-cairn"
            "--bin"
            "kiln-cairn-intake"
          ];
          meta.description = "Admit complete Cairn implementation-ready snapshots into the durable Kiln work ledger";
        });
      # Fleet orchestration: claim authority, herdr dispatcher, MCP server and
      # client commands in one binary. Its tests drive git and tar.
      # r[impl kiln.fleet.api]
      kilnFleetPackageFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        (packageFor system).overrideAttrs (old: {
          pname = "kiln-fleet";
          cargoBuildFlags = [
            "-p"
            "kiln-fleet"
            "--bin"
            "kiln-fleet"
          ];
          cargoTestFlags = [
            "-p"
            "kiln-fleet"
          ];
          nativeCheckInputs = (old.nativeCheckInputs or [ ]) ++ [
            pkgs.git
            pkgs.gnutar
          ];
          postInstall = "";
          meta = {
            description = "Kiln fleet orchestration: claim authority and herdr dispatcher for interactive agents";
            mainProgram = "kiln-fleet";
          };
        });
      # Reviewed guest kernel shared by every Kiln microVM guest image: the
      # pinned guest nixpkgs kernel, its stripped vmlinux and a plain-module
      # closure for an explicit root module list.
      guestKernelFor =
        system:
        let
          pkgs = import guest-nixpkgs { inherit system; };
          # r[impl kiln.hosted.guest.kernel.fixed]
          # Issue #28's fixed-series admission table, not independent CVE proof.
          fixedKernelMinimums = {
            "7.2" = "7.2.10";
            "6.18" = "6.18.56";
            "6.12" = "6.12.113";
            "6.6" = "6.6.159";
            "6.1" = "6.1.190";
            "5.15" = "5.15.223";
            "5.10" = "5.10.272";
          };
          guestKernel = pkgs.linuxPackages.kernel;
          guestKernelSeries = pkgs.lib.versions.majorMinor guestKernel.version;
          guestKernelFixed =
            pkgs.lib.versionAtLeast guestKernel.version "7.3"
            || (
              builtins.hasAttr guestKernelSeries fixedKernelMinimums
              && pkgs.lib.versionAtLeast guestKernel.version fixedKernelMinimums.${guestKernelSeries}
            );
          kernel =
            assert pkgs.lib.assertMsg guestKernelFixed
              "Kiln guest kernel ${guestKernel.version} is below the reviewed issue #28 fixed-series threshold";
            guestKernel;
        in
        {
          inherit pkgs kernel;
          vmlinux = pkgs.runCommand "kiln-microvm-vmlinux" { } ''
            ${pkgs.binutils-unwrapped}/bin/strip --strip-all -o "$out" ${kernel.dev}/vmlinux
          '';
          # Busybox modprobe loads plain modules; decompress and re-index here.
          modulesFor =
            name: rootModules:
            let
              modulesClosure = pkgs.makeModulesClosure {
                kernel = kernel.modules;
                firmware = kernel.modules;
                inherit rootModules;
                allowMissing = false;
              };
            in
            pkgs.runCommand name
              {
                nativeBuildInputs = [
                  pkgs.xz
                  pkgs.kmod
                ];
              }
              ''
                mkdir -p "$out/lib"
                cp -r --no-preserve=mode ${modulesClosure}/lib/modules "$out/lib/"
                find "$out/lib/modules" -name '*.ko.xz' -exec xz -d {} +
                depmod -b "$out" ${kernel.modDirVersion}
              '';
        };
      # Hosted Firecracker backend guest: the pinned nixpkgs (NixOS) kernel, a
      # makeInitrdNG initramfs rootfs and the guest agent. Not a full NixOS system.
      # r[impl kiln.hosted.microvm.protocol]
      # r[impl kiln.hosted.microvm.escape]
      # r[impl kiln.hosted.microvm.escape.scenario.suite]
      microvmGuestFor =
        system: vmm:
        let
          guestKernel = guestKernelFor system;
          inherit (guestKernel) pkgs kernel vmlinux;
          vmmPackage = if vmm == "firecracker" then pkgs.firecracker else pkgs.cloud-hypervisor;
          vmmBinary = "${vmmPackage}/bin/${
            if vmm == "firecracker" then "firecracker" else "cloud-hypervisor"
          }";
          agent = (packageFor system).overrideAttrs (_: {
            pname = "kiln-microvm-guest-agent";
            cargoBuildFlags = [
              "-p"
              "kiln-microvm"
              "--bin"
              "kiln-microvm-guest"
            ];
            cargoTestFlags = [
              "-p"
              "kiln-microvm"
            ];
          });
          rootModules = [
            (if vmm == "firecracker" then "virtio_mmio" else "virtio_pci")
            "virtio_blk"
            "vmw_vsock_virtio_transport"
            "virtio_rng"
            "vmgenid"
          ];
          modules = guestKernel.modulesFor "kiln-microvm-guest-modules" rootModules;
          # Modules load before kernel.modules_disabled; every namespace kind is
          # disabled; the agent becomes PID 1 and runs steps as uid/gid 1000.
          init = pkgs.writeScript "kiln-microvm-init" ''
            #!/bin/busybox sh
            set -eu
            /bin/busybox mkdir -p /proc /sys /dev /work /tmp
            /bin/busybox mount -t proc -o nosuid,nodev,noexec proc /proc
            /bin/busybox --install -s /bin
            mount -t sysfs -o nosuid,nodev,noexec sysfs /sys
            mount -t devtmpfs -o nosuid,noexec devtmpfs /dev
            for module in ${pkgs.lib.concatStringsSep " " rootModules}; do
              modprobe "$module"
            done
            echo 1 > /proc/sys/kernel/modules_disabled
            for kind in user mnt pid net ipc uts cgroup time; do
              echo 0 > "/proc/sys/user/max_''${kind}_namespaces"
            done
            echo 1 > /proc/sys/kernel/dmesg_restrict
            echo 2 > /proc/sys/kernel/kptr_restrict
            mount -t tmpfs -o mode=0755,nosuid,nodev tmpfs /work
            mount -t tmpfs -o mode=1777,nosuid,nodev tmpfs /tmp
            exec /bin/kiln-microvm-guest serve /dev/vda /bin/busybox
          '';
          initrd = pkgs.makeInitrdNG {
            name = "kiln-microvm-guest-initrd";
            compressor = "cat";
            contents = [
              {
                source = init;
                target = "/init";
              }
              {
                source = "${pkgs.busybox}/bin/busybox";
                target = "/bin/busybox";
              }
              {
                source = "${pkgs.bash}/bin/bash";
                target = "/bin/bash";
              }
              {
                source = "${agent}/bin/kiln-microvm-guest";
                target = "/bin/kiln-microvm-guest";
              }
              {
                source = "${modules}/lib/modules";
                target = "/lib/modules";
              }
            ];
          };
        in
        pkgs.runCommand (if vmm == "firecracker" then "kiln-microvm-guest" else "kiln-microvm-guest-ch")
          {
            nativeBuildInputs = [ pkgs.b3sum ];
            meta.description =
              if vmm == "firecracker" then
                "Firecracker guest kernel, initramfs and pinned VMM for the hosted microVM backend"
              else
                "Cloud Hypervisor guest kernel, initramfs and pinned VMM for the hosted microVM backend";
          }
          ''
            mkdir -p "$out"
            ln -s ${vmlinux} "$out/vmlinux"
            ln -s ${initrd}/initrd "$out/initrd"
            ln -s ${vmmBinary} "$out/vmm"
            if [ "${vmm}" = "firecracker" ]; then
              ln -s ${vmmBinary} "$out/firecracker"
            else
              ln -s ${vmmBinary} "$out/cloud-hypervisor"
            fi
            entry() {
              printf '{"path":"%s","blake3":"%s"}' "$1" "$(b3sum --no-names "$1")"
            }
            if [ "${vmm}" = "firecracker" ]; then
              vmm_entry="\"firecracker\":$(entry ${vmmBinary})"
            else
              vmm_entry="\"cloud-hypervisor\":$(entry ${vmmBinary})"
            fi
            cat > "$out/manifest.json" <<EOF
            {"schema":"kiln.microvm.guest-image.v2","kernel_version":"${kernel.version}","vmm":{"kind":"${vmm}","version":"${vmmPackage.version}","binary":$(entry ${vmmBinary})},$vmm_entry,"guest_shell":"/bin/bash","kernel":$(entry ${vmlinux}),"initrd":$(entry "$(readlink -f ${initrd}/initrd)")}
            EOF
          '';
      # r[impl kiln.lattice.worker.guest] r[impl kiln.lattice.worker.guest.scenario.identity]
      # Pinned Lattice worker guest (Kiln #51 M1): the reviewed guest kernel, an
      # immutable initramfs with the exact Lattice executable, admitted workflow
      # definitions and the Kiln guest shell, the pinned Cloud Hypervisor and a
      # `kiln.lattice-worker-guest.v1` manifest. Workspace, Lattice state, guest
      # identity and effect authority are per attempt and never in the image.
      latticeWorkerGuestFor =
        system:
        let
          guestKernel = guestKernelFor system;
          inherit (guestKernel) pkgs kernel vmlinux;
          inherit (latticeWorkerCohort) lattice;
          latticeBinary = "${lattice}/bin/lattice";
          vmmBinary = "${pkgs.cloud-hypervisor}/bin/cloud-hypervisor";
          guestShell = (packageFor system).overrideAttrs (_: {
            pname = "kiln-lattice-worker-guest";
            cargoBuildFlags = [
              "-p"
              "kiln-microvm"
              "--bin"
              "kiln-lattice-worker-guest"
            ];
            cargoTestFlags = [
              "-p"
              "kiln-microvm"
            ];
          });
          guestShellBinary = "${guestShell}/bin/kiln-lattice-worker-guest";
          workflowNames = [
            "log-triage"
            "workspace-journal"
          ];
          workflows = pkgs.runCommand "kiln-lattice-worker-workflows" { } ''
            mkdir -p "$out"
            cp ${latticeWorkerCohort.source}/examples/workflows/log-triage.ncl "$out/log-triage.ncl"
            cp ${./fixtures/lattice-worker/workspace-journal.ncl} "$out/workspace-journal.ncl"
          '';
          # The pinned Lattice binary reports each workflow's exact effect
          # requirements; the guest re-inspects and refuses any difference.
          environment =
            pkgs.runCommand "kiln-lattice-worker-environment"
              {
                nativeBuildInputs = [
                  pkgs.b3sum
                  pkgs.jq
                ];
              }
              ''
                export HOME="$TMPDIR/home"
                mkdir -p "$HOME"
                printf '{ data_dir = "%s", effect_authority = "%s", default_timeout_secs = 60, shell = "sh" }\n' \
                  "$TMPDIR/state" "$TMPDIR/absent.ncl" > "$TMPDIR/config.ncl"
                echo '{}' > workflows.json
                for name in ${pkgs.lib.concatStringsSep " " workflowNames}; do
                  ${latticeBinary} --config "$TMPDIR/config.ncl" import "${workflows}/$name.ncl" > /dev/null
                  ${latticeBinary} --config "$TMPDIR/config.ncl" authority inspect "$name" > "$name.requirements.json"
                  jq --arg name "$name" \
                    --arg definition "$(b3sum --no-names "${workflows}/$name.ncl")" \
                    --slurpfile requirements "$name.requirements.json" \
                    '.[$name] = {definition: $definition, requirements: $requirements[0]}' \
                    workflows.json > next.json
                  mv next.json workflows.json
                done
                mkdir -p "$out"
                jq -n \
                  --arg revision ${latticeWorkerCohort.revision} \
                  --arg lattice "$(b3sum --no-names ${latticeBinary})" \
                  --arg shell "$(b3sum --no-names ${guestShellBinary})" \
                  --arg busybox "$(b3sum --no-names ${pkgs.busybox}/bin/busybox)" \
                  --slurpfile workflows workflows.json \
                  '{schema: "kiln.lattice-worker-environment.v1", architecture: "x86_64",
                    lattice: {revision: $revision, binary: $lattice}, guest_shell: $shell,
                    busybox: $busybox, workflows: $workflows[0]}' > "$out/environment.json"
                ${guestShellBinary} environment-identity "$out/environment.json" > "$out/identity"
              '';
          rootModules = [
            "virtio_pci"
            "virtio_blk"
            "vmw_vsock_virtio_transport"
            "virtio_rng"
            "vmgenid"
            "ext4"
          ];
          modules = guestKernel.modulesFor "kiln-lattice-worker-modules" rootModules;
          # Same hardening as the per-job guest: modules load before
          # kernel.modules_disabled and every namespace kind is disabled. The
          # guest shell runs as PID 1's child and Lattice as uid/gid 1000.
          init = pkgs.writeScript "kiln-lattice-worker-init" ''
            #!/bin/busybox sh
            set -eu
            /bin/busybox mkdir -p /proc /sys /dev /tmp /workspace
            /bin/busybox mount -t proc -o nosuid,nodev,noexec proc /proc
            /bin/busybox --install -s /bin
            mount -t sysfs -o nosuid,nodev,noexec sysfs /sys
            mount -t devtmpfs -o nosuid,noexec devtmpfs /dev
            for module in ${pkgs.lib.concatStringsSep " " rootModules}; do
              modprobe "$module"
            done
            echo 1 > /proc/sys/kernel/modules_disabled
            for kind in user mnt pid net ipc uts cgroup time; do
              echo 0 > "/proc/sys/user/max_''${kind}_namespaces"
            done
            echo 1 > /proc/sys/kernel/dmesg_restrict
            echo 2 > /proc/sys/kernel/kptr_restrict
            mount -t tmpfs -o mode=1777,nosuid,nodev tmpfs /tmp
            exec /bin/kiln-lattice-worker-guest serve
          '';
          initrdContents = [
            {
              source = init;
              target = "/init";
            }
            {
              source = "${pkgs.busybox}/bin/busybox";
              target = "/bin/busybox";
            }
            {
              source = guestShellBinary;
              target = "/bin/kiln-lattice-worker-guest";
            }
            {
              source = latticeBinary;
              target = "/bin/lattice";
            }
            {
              source = "${environment}/environment.json";
              target = "/etc/kiln/lattice-worker-environment.json";
            }
            {
              source = "${modules}/lib/modules";
              target = "/lib/modules";
            }
          ]
          ++ map (name: {
            source = "${workflows}/${name}.ncl";
            target = "/etc/lattice/workflows/${name}.ncl";
          }) workflowNames;
          initrd = pkgs.makeInitrdNG {
            name = "kiln-lattice-worker-initrd";
            compressor = "cat";
            contents = initrdContents;
          };
          # Bounded guest resources; the initramfs carries the Lattice closure.
          vcpus = 1;
          memoryMib = 768;
          workspaceMib = 64;
        in
        pkgs.runCommand "kiln-lattice-worker-guest"
          {
            nativeBuildInputs = [
              pkgs.b3sum
              pkgs.jq
            ];
            meta.description = "Pinned Lattice worker guest: kernel, immutable initramfs, Cloud Hypervisor and manifest";
          }
          ''
            mkdir -p "$out"
            ln -s ${vmlinux} "$out/vmlinux"
            ln -s ${initrd}/initrd "$out/initrd"
            ln -s ${vmmBinary} "$out/cloud-hypervisor"
            ln -s ${environment}/environment.json "$out/environment.json"
            version="$(${vmmBinary} --version)"
            object() {
              jq -n --arg path "$1" --arg blake3 "$(b3sum --no-names "$1")" '{path: $path, blake3: $blake3}'
            }
            jq -n \
              --argjson kernel "$(object ${vmlinux})" \
              --argjson initrd "$(object "$(readlink -f ${initrd}/initrd)")" \
              --argjson binary "$(object ${vmmBinary})" \
              --arg version "$version" \
              --slurpfile environment ${environment}/environment.json \
              --arg identity "$(cat ${environment}/identity)" \
              '{schema: "kiln.lattice-worker-guest.v1", architecture: "x86_64",
                kernel: $kernel, initrd: $initrd,
                vmm: {kind: "cloud-hypervisor", version: $version, binary: $binary},
                environment: $environment[0], environment_identity: $identity,
                devices: {workspace_disk: "virtio-blk-raw", vsock_port: 10241, network: "none"},
                resources: {vcpus: ${toString vcpus}, memory_mib: ${toString memoryMib},
                  workspace_mib: ${toString workspaceMib}}}' > "$out/manifest.json"
            echo "${kernel.version}" > "$out/kernel-version"
          '';
      # r[impl kiln.hosted.microvm.guest_surface]
      # Measure the pinned guest kernel's compiled-in driver surface against the
      # reviewed deny list. The checked-in inventory pins the current cohort;
      # refresh it together with the guest nixpkgs pin.
      microvmGuestSurfaceFor =
        system: vmm:
        let
          pkgs = import guest-nixpkgs { inherit system; };
          kernel = pkgs.linuxPackages.kernel;
          rootModules =
            if vmm == "firecracker" then
              [
                "virtio_mmio"
                "virtio_blk"
                "vmw_vsock_virtio_transport"
                "virtio_rng"
                "vmgenid"
              ]
            else
              [
                "virtio_pci"
                "virtio_blk"
                "vmw_vsock_virtio_transport"
                "virtio_rng"
                "vmgenid"
              ];
          modulesClosure = pkgs.makeModulesClosure {
            kernel = kernel.modules;
            firmware = kernel.modules;
            inherit rootModules;
            allowMissing = false;
          };
        in
        pkgs.runCommand "kiln-microvm-guest-surface-${vmm}"
          {
            nativeBuildInputs = [
              pkgs.nickel
              pkgs.python3
              pkgs.b3sum
            ];
            meta.description = "Measured guest kernel driver surface and deny-list verdict for the pinned guest image";
          }
          ''
            mkdir -p "$out"
            nickel export --format json ${./config/microvm-guest-surface.ncl} > denylist.json
            python3 ${./scripts/microvm-guest-surface.py} \
              --denylist denylist.json \
              --config ${kernel.configfile} \
              --config-blake3 "$(b3sum --no-names ${kernel.configfile})" \
              --modules ${modulesClosure}/lib/modules \
              --kernel-version ${kernel.version} \
              --nixpkgs-revision ${guest-nixpkgs.rev} \
              --vmm ${vmm} \
              --out "$out/inventory.json"
            cmp "$out/inventory.json" ${./fixtures/microvm/guest-driver-surface.json}
          '';
      kilnAspenHostPackageFor =
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./runtime/kiln-aspen-host/rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
        in
        rustPlatform.buildRustPackage {
          pname = "kiln-aspen-host";
          version = "0.1.0";
          src = ./runtime/kiln-aspen-host;
          cargoLock = {
            lockFile = ./runtime/kiln-aspen-host/Cargo.lock;
            allowBuiltinFetchGit = true;
          };
          nativeBuildInputs = [
            pkgs.cmake
            pkgs.pkg-config
          ];
          buildInputs = [ pkgs.zlib ];
          preBuild = prepareKamaciteBuildSupport;
          cargoBuildFlags = [
            "--bin"
            "kiln-aspen-host"
          ];
          cargoTestFlags = [
            "--workspace"
            "--all-targets"
            "--all-features"
            "--"
            "--nocapture"
          ];
        };
      kilnRadicleNixProviderPackageFor =
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./runtime/kiln-radicle-nix-provider/rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
        in
        rustPlatform.buildRustPackage {
          pname = "kiln-radicle-nix-provider";
          version = "0.1.0";
          src = ./runtime/kiln-radicle-nix-provider;
          cargoLock = {
            lockFile = ./runtime/kiln-radicle-nix-provider/Cargo.lock;
            allowBuiltinFetchGit = true;
          };
          cargoBuildFlags = [
            "--bin"
            "kiln-radicle-nix-provider"
          ];
          cargoTestFlags = [
            "--workspace"
            "--all-targets"
            "--all-features"
            "--"
            "--nocapture"
          ];
        };
      # r[impl kiln.dashboard.live.supervision]
      # r[impl kiln.dashboard.live.supervision.scenario.cadence]
      dashboardToolsPackageFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        pkgs.stdenvNoCC.mkDerivation {
          pname = "kiln-dashboard-tools";
          version = "0.1.0";
          dontUnpack = true;
          nativeBuildInputs = [ pkgs.makeWrapper ];
          installPhase = ''
            runHook preInstall
            mkdir -p "$out/bin"
            install -Dm644 ${./scripts/dashboard-refresh.py} "$out/libexec/kiln-dashboard/dashboard-refresh.py"
            install -Dm644 ${./scripts/dashboard-publish.py} "$out/libexec/kiln-dashboard/dashboard-publish.py"
            for tool in refresh publish; do
              makeWrapper ${pkgs.python3}/bin/python3 "$out/bin/kiln-dashboard-$tool" \
                --add-flags "-I -B $out/libexec/kiln-dashboard/dashboard-$tool.py" \
                --prefix PATH : ${
                  pkgs.lib.makeBinPath [
                    pkgs.rsync
                    pkgs.openssh
                  ]
                }
            done
            install -Dm644 ${./config/systemd/kiln-dashboard-refresh.service} \
              "$out/share/systemd/user/kiln-dashboard-refresh.service"
            install -Dm644 ${./config/systemd/kiln-dashboard-refresh.timer} \
              "$out/share/systemd/user/kiln-dashboard-refresh.timer"
            runHook postInstall
          '';
          meta.description = "Finite read-only dashboard collection and atomic HTML publication tools";
        };

      # r[impl kiln.dashboard.rats.provenance]
      # r[verify kiln.dashboard.rats.provenance]
      dashboardIndexPackageFor =
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./web/dashboard-index/rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
        in
        rustPlatform.buildRustPackage {
          pname = "kiln-dashboard-index-assets";
          version = "0.1.0";
          # Embed the same WASM bytes as the explicit browser dev shell build.
          # Native Kiln binaries retain their normal cargo-auditable policy.
          auditable = false;
          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./web/dashboard-index/Cargo.toml
              ./web/dashboard-index/Cargo.lock
              ./web/dashboard-index/rust-toolchain.toml
              ./web/dashboard-index/src
              ./web/dashboard-index/rat-table/Cargo.toml
              ./web/dashboard-index/loader.js
              ./web/dashboard-index/notices
              ./vendor/subwayrat
              ./vendor/subwayrat.source.json
              ./scripts/build-dashboard-index.py
              ./scripts/browser_wasm_assets.py
              ./crates/kiln-dashboard/src/render/generated
              ./LICENSE-MIT
              ./LICENSE-APACHE
            ];
          };
          cargoRoot = "web/dashboard-index";
          cargoLock.lockFile = ./web/dashboard-index/Cargo.lock;
          nativeBuildInputs = [
            pkgs.python3
            pkgs.wasm-bindgen-cli
            pkgs.esbuild
          ];
          buildPhase = ''
            runHook preBuild
            python3 scripts/build-dashboard-index.py --check
            runHook postBuild
          '';
          checkPhase = ''
            runHook preCheck
            cargo test --locked --offline --manifest-path web/dashboard-index/Cargo.toml --lib
            cargo clippy --locked --offline --manifest-path web/dashboard-index/Cargo.toml \
              --target wasm32-unknown-unknown -- -D warnings
            cargo fmt --manifest-path web/dashboard-index/Cargo.toml --package kiln-dashboard-index -- --check
            runHook postCheck
          '';
          installPhase = ''
            runHook preInstall
            mkdir -p "$out/share/kiln-dashboard-index"
            cp crates/kiln-dashboard/src/render/generated/* "$out/share/kiln-dashboard-index/"
            runHook postInstall
          '';
          meta.description = "Reproduced locked offline Rats/Ratzilla dashboard index assets";
        };

      # r[impl kiln.sdlc.herdr_viewer]
      herdrViewerPackageFor =
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./web/herdr-viewer/rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
        in
        rustPlatform.buildRustPackage {
          pname = "kiln-herdr-viewer-assets";
          version = "0.1.0";
          # Embed the same WASM bytes as the explicit browser dev shell build.
          auditable = false;
          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./web/herdr-viewer/Cargo.toml
              ./web/herdr-viewer/Cargo.lock
              ./web/herdr-viewer/rust-toolchain.toml
              ./web/herdr-viewer/src
              ./web/herdr-viewer/rio
              ./web/herdr-viewer/loader.js
              ./vendor/rio
              ./vendor/rio.source.json
              ./scripts/build-herdr-viewer.py
              ./scripts/browser_wasm_assets.py
              ./crates/kiln-web-service/src/generated
              ./LICENSE-MIT
              ./LICENSE-APACHE
            ];
          };
          cargoRoot = "web/herdr-viewer";
          cargoLock.lockFile = ./web/herdr-viewer/Cargo.lock;
          nativeBuildInputs = [
            pkgs.python3
            pkgs.wasm-bindgen-cli
            pkgs.esbuild
          ];
          buildPhase = ''
            runHook preBuild
            python3 scripts/build-herdr-viewer.py --check
            runHook postBuild
          '';
          checkPhase = ''
            runHook preCheck
            cargo test --locked --offline --manifest-path web/herdr-viewer/Cargo.toml --lib
            cargo clippy --locked --offline --manifest-path web/herdr-viewer/Cargo.toml \
              --target wasm32-unknown-unknown -- -D warnings
            cargo fmt --manifest-path web/herdr-viewer/Cargo.toml --package kiln-herdr-viewer -- --check
            runHook postCheck
          '';
          installPhase = ''
            runHook preInstall
            mkdir -p "$out/share/kiln-herdr-viewer"
            cp crates/kiln-web-service/src/generated/* "$out/share/kiln-herdr-viewer/"
            runHook postInstall
          '';
          meta.description = "Reproduced locked read-only Rio Herdr pane viewer assets";
        };
    in
    {
      lib.unikernelImage = args: (unikernelFor "x86_64-linux").mkImage args;
      # r[impl kiln.web.deploy.install]
      onixModules.kiln-web-service = {
        file = ./deployment/onixos-module.ncl;
      };

      packages = eachSystem (
        system:
        {
          kiln = packageFor system;
          cairn = cairnPackageFor system;
          kiln-cairn-intake = kilnCairnIntakePackageFor system;
          kiln-fleet = kilnFleetPackageFor system;
          web-service = webServicePackageFor system;
          implementation-fixtures = implementationFixturesPackageFor system;
          website-exploration = websiteExplorationPackageFor system;
          bombadil = bombadilPackageFor system;
          web-exploration-bombadil = webExplorationBombadilPackageFor system;
          kiln-aspen-host = kilnAspenHostPackageFor system;
          kiln-radicle-nix-provider = kilnRadicleNixProviderPackageFor system;
          dashboard-tools = dashboardToolsPackageFor system;
          dashboard-index-assets = dashboardIndexPackageFor system;
          herdr-viewer-assets = herdrViewerPackageFor system;
          celld-cohort = celldCohortPackageFor system;
          pijul-cohort-fixture-producer = pijulCohortPackageFor system;
          secretspec-cohort = secretspecPackageFor system;
          default = packageFor system;
        }
        # The Firecracker guest's CPU template and module set are x86_64-only.
        // nixpkgs.lib.optionalAttrs (system == "x86_64-linux") {
          microvm-guest = microvmGuestFor system "firecracker";
          lattice-worker-guest = latticeWorkerGuestFor system;
          microvm-guest-ch = microvmGuestFor system "cloud-hypervisor";
          microvm-guest-surface = microvmGuestSurfaceFor system "firecracker";
          unikernel-guest = (unikernelFor system).guest;
          unikernel-rust-image = (unikernelFor system).rustImage;
          unikernel-go-image = (unikernelFor system).goImage;
          unikernel-probe-image = (unikernelFor system).probeImage;
        }
      );

      apps = eachSystem (system: {
        kiln = {
          type = "app";
          program = "${packageFor system}/bin/kiln";
          meta.description = "Run the Kiln local operator CLI";
        };
        kiln-adapter-github = {
          type = "app";
          program = "${packageFor system}/bin/kiln-adapter-github";
          meta.description = "Run the Kiln GitHub webhook and recovery service";
        };
        kiln-implementer = {
          type = "app";
          program = "${packageFor system}/bin/kiln-implementer";
          meta.description = "Drain authenticated GitHub issues into admitted implementation work";
        };
        kiln-lattice-dispatch = {
          type = "app";
          program = "${packageFor system}/bin/kiln-lattice-dispatch";
          meta.description = "Dispatch one reserved implementation attempt to an admitted Lattice request";
        };
        kiln-cairn-intake = {
          type = "app";
          program = "${kilnCairnIntakePackageFor system}/bin/kiln-cairn-intake";
          meta.description = "Admit complete Cairn implementation-ready snapshots into the durable Kiln work ledger";
        };
        kiln-fleet = {
          type = "app";
          program = "${kilnFleetPackageFor system}/bin/kiln-fleet";
          meta.description = "Kiln fleet orchestration: claim authority and herdr dispatcher for interactive agents";
        };
        kiln-adapter-gitea = {
          type = "app";
          program = "${packageFor system}/bin/kiln-adapter-gitea";
          meta.description = "Run the Kiln Gitea/Forgejo webhook and recovery service";
        };
        kiln-adapter-iroh = {
          type = "app";
          program = "${packageFor system}/bin/kiln-adapter-iroh";
          meta.description = "Run the authenticated Iroh native workflow worker or Unix proxy";
        };
        kiln-patchgate-review = {
          type = "app";
          program = "${packageFor system}/bin/kiln-patchgate-review";
          meta.description = "Run isolated Patchgate review and retain exact non-authorizing evidence";
        };
        kiln-github-job = {
          type = "app";
          program = "${packageFor system}/bin/kiln-github-job";
          meta.description = "Run a pinned exact-source GitHub CI job";
        };
        kiln-gitea-job = {
          type = "app";
          program = "${packageFor system}/bin/kiln-gitea-job";
          meta.description = "Run a pinned exact-source Gitea/Forgejo CI job";
        };
        kiln-radicle-ci = {
          type = "app";
          program = "${packageFor system}/bin/kiln-radicle-ci";
          meta.description = "Run source-bound Radicle CI and original-operation recovery";
        };
        kiln-dashboard = {
          type = "app";
          program = "${packageFor system}/bin/kiln-dashboard";
          meta.description = "Render the local operator dashboard over a canonical Kiln journal";
        };
        kiln-hosted = {
          type = "app";
          program = "${packageFor system}/bin/kiln-hosted";
          meta.description = "Run the explicitly admitted Iroh-connected hosted CI composition";
        };
        kiln-web-service = {
          type = "app";
          program = "${webServicePackageFor system}/bin/kiln-web-service";
          meta.description = "Run the authenticated source-bound single-tenant CI web service";
        };
        kiln-website-exploration = {
          type = "app";
          program = "${websiteExplorationPackageFor system}/bin/kiln-website-exploration";
          meta.description = "Explore the real console with isolated synthetic examples on IPv4 loopback";
        };
        bombadil = {
          type = "app";
          program = "${bombadilPackageFor system}/bin/bombadil";
          meta.description = "Run the existing pinned Bombadil property-testing tool";
        };
        kiln-web-exploration-bombadil = {
          type = "app";
          program = "${webExplorationBombadilPackageFor system}/bin/kiln-web-exploration-bombadil";
          meta.description = "Run bounded read-only browser properties on real Kiln website examples";
        };
        kiln-nix-cache = {
          type = "app";
          program = "${packageFor system}/bin/kiln-nix-cache";
          meta.description = "Render a bounded read-only Nix binary-cache projection";
        };
        kiln-dashboard-refresh = {
          type = "app";
          program = "${dashboardToolsPackageFor system}/bin/kiln-dashboard-refresh";
          meta.description = "Collect bounded canonical CI histories and publish one dashboard report";
        };
        kiln-dashboard-publish = {
          type = "app";
          program = "${dashboardToolsPackageFor system}/bin/kiln-dashboard-publish";
          meta.description = "Render and atomically deliver an explicitly configured dashboard catalog";
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
          # Every root check builds with the pinned file; the workspace rust-version
          # makes Cargo refuse an older compiler.
          # r[impl kiln.toolchain.pin] r[related kiln.toolchain.pin.scenario.older]
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
          # Upstream Radicle 0.25.1's isolated helper cohort: the minimal profile of
          # the same Rust 1.97.1 release that rust-toolchain.toml pins for core.
          radiclePatchToolchain = pkgs.rust-bin.stable."1.97.1".minimal;
          radiclePatchRustPlatform = pkgs.makeRustPlatform {
            cargo = radiclePatchToolchain;
            rustc = radiclePatchToolchain;
          };
          package = packageFor system;
          hostPackage = kilnAspenHostPackageFor system;
          radicleNixProviderPackage = kilnRadicleNixProviderPackageFor system;
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
          aspenRuntimeOctetWorkspace = pkgs.runCommand "kiln-aspen-runtime-octet-workspace" { } ''
            mkdir -p \
              "$out/crates/kiln-adapter-aspen" \
              "$out/crates/kiln-aspen-extension" \
              "$out/crates/kiln-core" \
              "$out/crates/kiln-provider-api" \
              "$out/crates/kiln-codec" \
              "$out/fixtures/aspen/runtime"
            cp ${./checks/aspen-runtime-octet/Cargo.toml} "$out/Cargo.toml"
            cp ${./checks/aspen-runtime-octet/Cargo.lock} "$out/Cargo.lock"
            cp ${./checks/aspen-runtime-octet/dylint.toml} "$out/dylint.toml"
            cp ${./crates/kiln-adapter-aspen/Cargo.toml} \
              "$out/crates/kiln-adapter-aspen/Cargo.toml"
            cp ${./crates/kiln-aspen-extension/Cargo.toml} \
              "$out/crates/kiln-aspen-extension/Cargo.toml"
            cp ${./crates/kiln-core/Cargo.toml} "$out/crates/kiln-core/Cargo.toml"
            cp ${./crates/kiln-provider-api/Cargo.toml} \
              "$out/crates/kiln-provider-api/Cargo.toml"
            cp ${./crates/kiln-codec/Cargo.toml} "$out/crates/kiln-codec/Cargo.toml"
            cp -R ${./crates/kiln-adapter-aspen/src} \
              "$out/crates/kiln-adapter-aspen/src"
            cp -R ${./crates/kiln-aspen-extension/src} \
              "$out/crates/kiln-aspen-extension/src"
            cp -R ${./crates/kiln-aspen-extension/tests} \
              "$out/crates/kiln-aspen-extension/tests"
            cp -R ${./crates/kiln-aspen-extension/examples} \
              "$out/crates/kiln-aspen-extension/examples"
            cp -R ${./fixtures/aspen/runtime}/* "$out/fixtures/aspen/runtime/"
            chmod -R u+w "$out"
            cat ${./checks/aspen-runtime-octet/adapter-header.rs} \
              "$out/crates/kiln-adapter-aspen/src/lib.rs" \
              > "$out/crates/kiln-adapter-aspen/src/lib.rs.combined"
            mv "$out/crates/kiln-adapter-aspen/src/lib.rs.combined" \
              "$out/crates/kiln-adapter-aspen/src/lib.rs"
            cat ${./checks/aspen-runtime-octet/extension-header.rs} \
              "$out/crates/kiln-aspen-extension/src/lib.rs" \
              > "$out/crates/kiln-aspen-extension/src/lib.rs.combined"
            mv "$out/crates/kiln-aspen-extension/src/lib.rs.combined" \
              "$out/crates/kiln-aspen-extension/src/lib.rs"
            prepend_header() {
              header="$1"
              target="$2"
              cat "$header" "$target" > "$target.combined"
              mv "$target.combined" "$target"
            }
            prepend_header ${./checks/aspen-runtime-octet/target-header.rs} \
              "$out/crates/kiln-aspen-extension/src/main.rs"
            prepend_header ${./checks/aspen-runtime-octet/target-header.rs} \
              "$out/crates/kiln-aspen-extension/examples/emitcallbackfixture.rs"
            prepend_header ${./checks/aspen-runtime-octet/test-header.rs} \
              "$out/crates/kiln-aspen-extension/tests/callback.rs"
            prepend_header ${./checks/aspen-runtime-octet/test-header.rs} \
              "$out/crates/kiln-aspen-extension/tests/hosted.rs"
            substituteInPlace "$out/Cargo.toml" \
              --replace-fail '@KILN_SOURCE@' '${self}'
          '';
          radicleBrokerWireOctetWorkspace = pkgs.runCommand "kiln-radicle-broker-wire-octet-workspace" { } ''
            mkdir -p \
              "$out/crates/kiln-radicle-broker-wire-octet/src/defelo" \
              "$out/fixtures/radicle-ci-broker/0.31.0"
            ${pkgs.gnugrep}/bin/grep -Fqx \
              'pub const MAX_LOG_URL_BYTES: usize = 2048;' \
              ${./crates/kiln-adapter-radicle/src/lib.rs}
            ${pkgs.gnugrep}/bin/grep -Fqx \
              'pub const MAX_LOG_URL_BYTES: usize = 2048;' \
              ${./checks/radicle-broker-wire-octet/lib.rs}
            ${pkgs.gnugrep}/bin/grep -Fqx \
              'pub const DEFAULT_MAX_POLLS: usize = 64;' \
              ${./crates/kiln-adapter-radicle/src/lib.rs}
            ${pkgs.gnugrep}/bin/grep -Fqx \
              'pub const DEFAULT_MAX_POLLS: usize = 64;' \
              ${./checks/radicle-broker-wire-octet/lib.rs}
            cp ${./checks/radicle-broker-wire-octet/Cargo.toml} "$out/Cargo.toml"
            cp ${./checks/radicle-broker-wire-octet/Cargo.lock} "$out/Cargo.lock"
            cp ${./checks/radicle-broker-wire-octet/dylint.toml} "$out/dylint.toml"
            cp ${./checks/radicle-broker-wire-octet/crate-Cargo.toml} \
              "$out/crates/kiln-radicle-broker-wire-octet/Cargo.toml"
            cp ${./checks/radicle-broker-wire-octet/lib.rs} \
              "$out/crates/kiln-radicle-broker-wire-octet/src/lib.rs"
            cp ${./crates/kiln-adapter-radicle/src/defelo.rs} \
              "$out/crates/kiln-radicle-broker-wire-octet/src/defelo.rs"
            cp ${./crates/kiln-adapter-radicle/src/defelo/tests.rs} \
              "$out/crates/kiln-radicle-broker-wire-octet/src/defelo/tests.rs"
            cp ${./fixtures/radicle-ci-broker/0.31.0/trigger.json} \
              "$out/fixtures/radicle-ci-broker/0.31.0/trigger.json"
            chmod -R u+w "$out"
            substituteInPlace "$out/Cargo.toml" \
              --replace-fail '@KILN_SOURCE@' '${self}'
          '';
        in
        {
          inherit package;
          # The checked-in inventory is x86_64 guest-cohort evidence.
        }
        // pkgs.lib.optionalAttrs (system == "x86_64-linux") {
          # r[verify kiln.hosted.microvm.guest_surface.scenario.absent] r[verify kiln.hosted.microvm.guest_surface.scenario.violation]
          microvm-guest-surface = microvmGuestSurfaceFor system "firecracker";
        }
        // {
          tests = package;
          boundaries = package;
          runtime-independence = package;
          dashboard-index-assets = dashboardIndexPackageFor system;
          herdr-viewer-assets = herdrViewerPackageFor system;
          # r[verify kiln.web.deploy.install.scenario.install] r[verify kiln.web.deploy.install.scenario.denied]
          # r[verify kiln.web.deploy.runtime.scenario.ready] r[verify kiln.web.deploy.runtime.scenario.denied]
          # r[impl kiln.web.deploy.evidence]
          web-service-deployment = import ./checks/web-service-deployment.nix {
            inherit pkgs;
            package = webServicePackageFor system;
          };
          format = pkgs.runCommand "kiln-format" { nativeBuildInputs = [ toolchain ]; } ''
            ${copySource}
            # Check every member and local path dependency. Pinned producer
            # trees disable formatting through their own rustfmt configuration.
            cargo fmt --all -- --check
            touch "$out"
          '';
          # Clippy from the pinned toolchain, warnings denied, over every target.
          # r[verify kiln.toolchain.gates] r[verify kiln.toolchain.gates.scenario.lint]
          clippy = rustPlatform.buildRustPackage {
            pname = "kiln-clippy";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            nativeBuildInputs = [
              toolchain
              pkgs.cmake
              pkgs.pkg-config
            ];
            buildInputs = [
              pkgs.zlib
              pkgs.openssl
            ];
            buildPhase = ''
              runHook preBuild
              cargo clippy --workspace --all-targets --all-features -- -D warnings
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
          # r[impl kiln.radicle.canonical.provenance] r[impl kiln.radicle.canonical.rail]
          # r[verify kiln.radicle.canonical.rail.scenario.tests]
          # r[verify kiln.radicle.canonical.provenance.scenario.refresh]
          # Vendored heartwood canonical computation: its own workspace, exact
          # upstream files plus documented shims, and its own lock file.
          heartwood-canonical = rustPlatform.buildRustPackage {
            pname = "heartwood-canonical";
            version = "0.1.0";
            src = ./vendor/heartwood-canonical;
            cargoLock.lockFile = ./vendor/heartwood-canonical/Cargo.lock;
            nativeBuildInputs = [
              pkgs.cmake
              pkgs.pkg-config
            ];
            buildInputs = [
              pkgs.zlib
              pkgs.openssl
            ];
            doCheck = true;
          };
          github-protocol-fixtures = rustPlatform.buildRustPackage {
            pname = "kiln-github-protocol-fixtures";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [
              pkgs.cmake
              pkgs.pkg-config
              pkgs.nickel
              pkgs.git
              pkgs.b3sum
            ];
            buildInputs = [
              pkgs.zlib
              pkgs.openssl
            ];
            buildPhase = ''
              runHook preBuild
              nickel export --format json fixtures/github/service-profile.ncl > github-service.json
              nickel export --format json fixtures/github/job-profile.ncl > github-job.json
              cargo test --locked -p kiln-adapter-github --features protocol-fixtures --offline
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
          gitea-protocol-fixtures = rustPlatform.buildRustPackage {
            pname = "kiln-gitea-protocol-fixtures";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [
              pkgs.cmake
              pkgs.pkg-config
              pkgs.nickel
              pkgs.git
              pkgs.b3sum
            ];
            buildInputs = [
              pkgs.zlib
              pkgs.openssl
            ];
            buildPhase = ''
              runHook preBuild
              nickel export --format json fixtures/gitea/service-profile.ncl > gitea-service.json
              nickel export --format json fixtures/gitea/job-profile.ncl > gitea-job.json
              cargo test --locked -p kiln-adapter-gitea --features protocol-fixtures --offline
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
          kiln-radicle-nix-provider-package = radicleNixProviderPackage;
          kiln-radicle-nix-provider-octet-deny-all =
            assert octetInput.rev == "cf04e894e53eb0947230118a086ef6066ddba38c";
            (octetInput.lib.mkConsumerCheck {
              inherit system;
              src = ./runtime/kiln-radicle-nix-provider;
              packages = [ "kiln-radicle-nix-provider" ];
              cargoExtraArgs = "--all-targets --all-features";
              cargoLock = ./runtime/kiln-radicle-nix-provider/Cargo.lock;
            }).overrideAttrs
              (_previous: {
                DYLINT_RUSTFLAGS = "--deny warnings";
              });
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
                  cargo test -p kiln-adapter-pijul --features native-fixtures --tests --offline
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
          # Change-ID intake over fixture repositories built by the pinned jj.
          # r[verify kiln.review.jj_intake.read_only]
          jj-native-fixtures = rustPlatform.buildRustPackage {
            pname = "kiln-jj-native-fixtures";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            nativeBuildInputs = [
              pkgs.cmake
              pkgs.pkg-config
            ];
            buildInputs = [
              pkgs.zlib
              pkgs.openssl
            ];
            buildPhase = ''
              runHook preBuild
              export KILN_JJ_EXE=${pkgs.jujutsu}/bin/jj
              cargo test -p kiln-adapter-jj --features jj-fixtures --offline
              cargo test -p kiln-web-service --features jj-fixtures --lib jj_intake --offline
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
          # Planned build-input views built by the pinned Cargo toolchain.
          # r[verify kiln.source.projection.build_inputs.scenario.qualified]
          build-input-fixtures = rustPlatform.buildRustPackage {
            pname = "kiln-build-input-fixtures";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            nativeBuildInputs = [
              pkgs.cmake
              pkgs.pkg-config
            ];
            buildInputs = [
              pkgs.zlib
              pkgs.openssl
            ];
            buildPhase = ''
              runHook preBuild
              KILN_CARGO_EXE=$(command -v cargo)
              export KILN_CARGO_EXE
              cargo test -p kiln-web-service --features build-input-fixtures --test build_input_qualification --offline
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
          # Exact Git acquisition (SHA-1 and SHA-256) and the loopback spindle flow.
          # r[verify kiln.tangled.source]
          tangled-git-fixtures = rustPlatform.buildRustPackage {
            pname = "kiln-tangled-git-fixtures";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            nativeBuildInputs = [
              pkgs.cmake
              pkgs.pkg-config
            ];
            buildInputs = [
              pkgs.zlib
              pkgs.openssl
            ];
            buildPhase = ''
              runHook preBuild
              export KILN_GIT_EXE=${pkgs.git}/bin/git
              cargo test -p kiln-adapter-tangled --features git-fixtures --offline
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
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
                    '/nix/store/wlm2nasarbcaji3y0v4gz39ckvqdmhgn-pijul-1.0.0-beta.11/bin/pijul'
                  test "$(b3sum --no-names "$executable")" = \
                    '22cf118c9ccb841a758b9d04a4724cf5e277273a44d47eb7f9d47d2fd43cb182'
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
          nix-build-adapter-fixtures = rustPlatform.buildRustPackage {
            pname = "kiln-nix-build-adapter-fixtures";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            buildPhase = ''
              runHook preBuild
              cargo test -p kiln-nix-store --offline
              cargo test -p kiln-adapter-nix --offline
              cargo test -p kiln-nix-build --offline
              runHook postBuild
            '';
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
          nix-build-adapter-cohort =
            pkgs.runCommand "kiln-nix-build-adapter-cohort"
              {
                nativeBuildInputs = [
                  pkgs.b3sum
                  pkgs.coreutils
                  pkgs.nix
                ];
              }
              ''
                test '${pkgs.nix}/bin/nix' = \
                  '/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix'
                test '${pkgs.nix}/bin/nix-store' = \
                  '/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix-store'
                test "$(b3sum --no-names ${pkgs.nix}/bin/nix)" = \
                  '82a5990db42a7f104b2c6eb17edf840ac9a7ad04936c8ab2630cc766e929f852'
                test "$(b3sum --no-names ${pkgs.nix}/bin/nix-store)" = \
                  '82a5990db42a7f104b2c6eb17edf840ac9a7ad04936c8ab2630cc766e929f852'
                test "$(${pkgs.nix}/bin/nix --version 2>/dev/null)" = 'nix (Nix) 2.34.8'
                touch "$out"
              '';
          octet =
            pkgs.runCommand "kiln-octet"
              {
                nativeBuildInputs = [
                  self.checks.${system}.aspen-runtime-octet-deny-all
                  self.checks.${system}.kiln-aspen-host-octet-deny-all
                  self.checks.${system}.kiln-radicle-nix-provider-octet-deny-all
                  self.checks.${system}.radicle-broker-wire-octet-deny-all
                ];
              }
              ''
                touch "$out"
              '';
          no-std = rustPlatform.buildRustPackage {
            pname = "kiln-no-std";
            version = "0.1.0";
            src = self;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            cargoBuildFlags = [
              "-p"
              "onix-kiln-core"
              "-p"
              "kiln-observe-core"
              "-p"
              "kiln-radicle-source-contract"
              "--target=wasm32-unknown-unknown"
            ];
            doCheck = false;
            installPhase = ''
              touch "$out"
            '';
          };
          kiln-aspen-host-package = hostPackage;
          kiln-aspen-host-octet-deny-all =
            assert octetInput.rev == "cf04e894e53eb0947230118a086ef6066ddba38c";
            (octetInput.lib.mkConsumerCheck {
              inherit system;
              src = ./runtime/kiln-aspen-host;
              packages = [ "kiln-aspen-host" ];
              cargoExtraArgs = "--all-targets --all-features";
              cargoLock = ./runtime/kiln-aspen-host/Cargo.lock;
              nativeBuildInputs = [
                pkgs.cmake
                pkgs.pkg-config
              ];
              buildInputs = [ pkgs.zlib ];
            }).overrideAttrs
              (
                previous:
                let
                  octetCommand = "${octetInput.packages.${system}.cargo-octet}/bin/cargo-octet check";
                  octetNeedle = builtins.unsafeDiscardStringContext octetCommand;
                  patchedOctetCommand = ''
                    first_vendor_directory="$(${pkgs.gnused}/bin/sed -n 's/^directory = "\(.*\)"$/\1/p' "$CARGO_HOME/config.toml" | ${pkgs.coreutils}/bin/head -n 1)"
                    test -n "$first_vendor_directory"
                    vendor_directory="$(${pkgs.coreutils}/bin/dirname "$first_vendor_directory")"
                    patched_vendor="$NIX_BUILD_TOP/cargo-vendor-patched"
                    ${pkgs.coreutils}/bin/mkdir -p "$patched_vendor"
                    ${pkgs.coreutils}/bin/cp -rs "$vendor_directory"/. "$patched_vendor"
                    kamacite_source="$(${pkgs.findutils}/bin/find -L "$vendor_directory" -type d -name kamacite-core-0.1.0 -print -quit)"
                    test -n "$kamacite_source"
                    kamacite_vendor_directory="$(${pkgs.coreutils}/bin/dirname "$kamacite_source")"
                    kamacite_vendor_key="$(${pkgs.coreutils}/bin/basename "$kamacite_vendor_directory")"
                    ${pkgs.coreutils}/bin/rm "$patched_vendor/$kamacite_vendor_key"
                    ${pkgs.coreutils}/bin/mkdir -p "$patched_vendor/$kamacite_vendor_key"
                    ${pkgs.coreutils}/bin/cp -rL "$kamacite_source" "$patched_vendor/$kamacite_vendor_key/kamacite-core-0.1.0"
                    ${pkgs.coreutils}/bin/mkdir -p "$patched_vendor/adapters/spacewasm-mvp/generated"
                    ${pkgs.coreutils}/bin/cp \
                      ${kamacite-build-support}/adapters/spacewasm-mvp/generated/profile.json \
                      "$patched_vendor/adapters/spacewasm-mvp/generated/profile.json"
                    ${pkgs.gnused}/bin/sed -i \
                      "s|$vendor_directory|$patched_vendor|" \
                      "$CARGO_HOME/config.toml"
                    ${octetCommand} \
                  '';
                in
                {
                  DYLINT_RUSTFLAGS = "--deny warnings";
                  buildCommand =
                    assert pkgs.lib.hasInfix octetNeedle previous.buildCommand;
                    builtins.replaceStrings [ octetNeedle ] [ patchedOctetCommand ] previous.buildCommand;
                }
              );
          aspen-runtime-octet-deny-all =
            assert octetInput.rev == "cf04e894e53eb0947230118a086ef6066ddba38c";
            (octetInput.lib.mkConsumerCheck {
              inherit system;
              src = aspenRuntimeOctetWorkspace;
              packages = [
                "kiln-adapter-aspen"
                "kiln-aspen-extension"
              ];
              # Octet lints root crates with its own pinned nightly (rustc 1.96),
              # below the workspace's tested rust-version. That declaration is a
              # compiler claim for Kiln builds, not a lint-gate requirement.
              cargoExtraArgs = "--all-targets --all-features --ignore-rust-version";
              cargoLock = ./checks/aspen-runtime-octet/Cargo.lock;
              nativeBuildInputs = [ pkgs.cmake ];
              buildInputs = [ pkgs.zlib ];
            }).overrideAttrs
              (_previous: {
                DYLINT_RUSTFLAGS = "--deny warnings";
              });
          radicle-broker-wire-octet-deny-all =
            assert octetInput.rev == "cf04e894e53eb0947230118a086ef6066ddba38c";
            (octetInput.lib.mkConsumerCheck {
              inherit system;
              src = radicleBrokerWireOctetWorkspace;
              packages = [ "kiln-radicle-broker-wire-octet" ];
              # Root crates under Octet's nightly; see aspen-runtime-octet-deny-all.
              cargoExtraArgs = "--all-targets --all-features --ignore-rust-version";
              cargoLock = ./checks/radicle-broker-wire-octet/Cargo.lock;
            }).overrideAttrs
              (_previous: {
                DYLINT_RUSTFLAGS = "--deny warnings";
              });
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
          onixos-pilot-preparation =
            pkgs.runCommand "kiln-onixos-pilot-preparation" { nativeBuildInputs = [ pkgs.python3 ]; }
              ''
                export PYTHONDONTWRITEBYTECODE=1
                python -m unittest discover -s ${self}/scripts -p test_onixos_ci_closure.py -v
                touch "$out"
              '';
          aspen-runtime-profile =
            pkgs.runCommand "kiln-aspen-runtime-profile" { nativeBuildInputs = [ pkgs.nickel ]; }
              ''
                ${copySource}
                nickel format --check \
                  config/aspen-runtime-profile.ncl \
                  config/fixtures/aspen-runtime-positive.ncl \
                  config/fixtures/negative-aspen-runtime-*.ncl
                nickel export config/aspen-runtime-profile.ncl >/dev/null
                nickel export config/fixtures/aspen-runtime-positive.ncl >/dev/null
                for fixture in config/fixtures/negative-aspen-runtime-*.ncl; do
                  if nickel export "$fixture" >/dev/null 2>&1; then
                    echo "negative Aspen runtime fixture passed: $fixture" >&2
                    exit 1
                  fi
                done
                touch "$out"
              '';
          semantic-merge-fixtures =
            pkgs.runCommand "kiln-semantic-merge-fixtures"
              {
                nativeBuildInputs = [
                  pkgs.diffutils
                  pkgs.nickel
                ];
              }
              ''
                ${copySource}
                cd fixtures/semantic-merge
                nickel format --check *.ncl
                nickel export --format json quartet.ncl > "$TMPDIR/quartet.json"
                diff -u quartet.json "$TMPDIR/quartet.json"
                for fixture in negative-*.ncl; do
                  if nickel export "$fixture" >/dev/null 2>&1; then
                    echo "negative semantic merge fixture passed: $fixture" >&2
                    exit 1
                  fi
                done
                touch "$out"
              '';
          nickel = pkgs.runCommand "kiln-nickel" { nativeBuildInputs = [ pkgs.nickel ]; } ''
            ${copySource}
            bash scripts/check-nickel-fixtures.sh
            sh tests/imessage-receiver-profile.sh
            nickel format --check \
              config/radicle-nix-provider.ncl \
              config/radicle-workflow-shell.ncl \
              config/fixtures/negative-radicle-workflow-shell-*.ncl \
              config/unikernel-preboot-seccomp.ncl \
              config/fixtures/radicle-nix-provider-positive.ncl \
              config/fixtures/negative-radicle-nix-provider-zero-user.ncl \
              config/nix-build-profile.ncl \
              config/fixtures/nix-build-positive.ncl \
              config/observation-profile.ncl \
              config/fixtures/observation-positive.ncl \
              config/fixtures/negative-observation-*.ncl \
              config/dashboard-profile.ncl \
              config/dashboard-catalog.ncl \
              config/dashboard-kiln.ncl \
              config/dashboard-publication.ncl \
              config/dashboard-refresh.ncl \
              config/fixtures/dashboard-catalog-*.ncl \
              config/fixtures/dashboard-publication-*.ncl \
              config/fixtures/dashboard-positive.ncl \
              config/fixtures/negative-dashboard-*.ncl \
              config/fixtures/dashboard-refresh-positive.ncl \
              config/web-service.ncl \
              config/web-worker.ncl \
              config/web-cache.ncl \
              config/web-workers.ncl \
              config/web-developer.ncl \
              config/fixtures/*web-cache*.ncl \
              config/fixtures/*web-workers*.ncl \
              config/fixtures/*web-worker-*.ncl \
              config/fixtures/*web-developer*.ncl \
              config/onixresearch-projects.ncl \
              config/web-systemd.ncl \
              config/web-access.ncl \
              config/web-automation.ncl \
              config/web-cobs.ncl \
              config/web-work.ncl \
              config/fixtures/*web-work-*.ncl \
              config/web-code-review.ncl \
              config/fixtures/*web-code-review-*.ncl \
              config/web-sources.ncl \
              config/fixtures/*web-access*.ncl \
              config/fixtures/*web-automation*.ncl \
              config/fixtures/*web-cobs*.ncl \
              config/fixtures/*web-sources*.ncl \
              config/fixtures/*web-systemd*.ncl \
              deployment/*.ncl \
              deployment/fixtures/*.ncl \
              config/fixtures/*web-service*.ncl \
              config/http-observation-profile.ncl \
              config/fixtures/http-observation-positive.ncl \
              config/fixtures/negative-http-observation-*.ncl \
              config/fixtures/ucan-observation-positive.ncl \
              config/fixtures/negative-ucan-*.ncl \
              config/iroh-observation-profile.ncl \
              config/fixtures/iroh-observation-positive.ncl \
              config/fixtures/negative-iroh-observation-*.ncl \
              config/patchgate-review.ncl \
              config/patchgate-review-job.ncl \
              config/fixtures/*patchgate-review*.ncl
            nickel export config/default.ncl >/dev/null
            nickel export --format json config/unikernel-preboot-seccomp.ncl > native-preboot-policy.json
            ${pkgs.python3}/bin/python3 -c 'import json,pathlib; policy=json.load(open("native-preboot-policy.json")); assert policy["policy"] == "kiln.native.preboot.v1" and policy["errno"] == 1; assert " ".join(policy["syscalls"])+"\n" == pathlib.Path("crates/kiln-hosted/src/unikernel-preboot-syscalls.txt").read_text()'
            nickel export config/hosting-profile.ncl >/dev/null
            nickel export config/cob-profiles.ncl >/dev/null
            nickel export config/git-source-profile.ncl >/dev/null
            nickel export config/pijul-source-profile.ncl >/dev/null
            nickel export config/lattice-workflow-profile.ncl >/dev/null
            nickel export config/cairn-gate-profile.ncl >/dev/null
            nickel typecheck config/patchgate-review.ncl
            nickel typecheck config/patchgate-review-job.ncl
            nickel format --check config/patchgate-review.ncl config/patchgate-review-job.ncl config/fixtures/*patchgate-review*.ncl
            nickel export config/fixtures/patchgate-review-job-positive.ncl >/dev/null
            for fixture in config/fixtures/negative-patchgate-review-job-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative Patchgate job fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel format --check config/hosted-*.ncl config/fixtures/*hosted*.ncl
            for profile in config/hosted-*.ncl; do
              nickel typecheck "$profile"
            done
            nickel export config/hosted-pipeline-example.ncl >/dev/null
            nickel export config/fixtures/hosted-pipeline-triggers-positive.ncl >/dev/null
            nickel export config/fixtures/hosted-service-positive.ncl >/dev/null
            nickel export config/fixtures/hosted-service-untrusted-positive.ncl >/dev/null
            nickel export config/fixtures/hosted-client-positive.ncl >/dev/null
            nickel typecheck config/hosted-lattice-job.ncl
            nickel export config/fixtures/hosted-lattice-job-positive.ncl >/dev/null
            nickel export config/fixtures/hosted-service-lattice-positive.ncl >/dev/null
            for fixture in config/fixtures/negative-hosted-service-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative hosted service fixture passed: $fixture" >&2
                exit 1
              fi
            done
            for fixture in config/fixtures/negative-hosted-lattice-job-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative hosted Lattice job fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel typecheck config/web-service.ncl
            nickel typecheck config/web-worker.ncl
            nickel typecheck config/web-cache.ncl
            nickel typecheck config/web-workers.ncl
            nickel typecheck config/web-developer.ncl
            for fixture in config/fixtures/web-cache-*.ncl config/fixtures/web-workers-*.ncl config/fixtures/web-worker-*.ncl config/fixtures/web-developer-*.ncl; do
              nickel export "$fixture" >/dev/null
            done
            for fixture in config/fixtures/negative-web-cache-*.ncl config/fixtures/negative-web-workers-*.ncl config/fixtures/negative-web-worker-*.ncl config/fixtures/negative-web-developer-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative developer-loop fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel export config/onixresearch-projects.ncl >/dev/null
            nickel typecheck config/web-systemd.ncl
            nickel typecheck config/web-cobs.ncl
            nickel export config/fixtures/web-access-positive.ncl >/dev/null
            for fixture in config/fixtures/negative-web-access-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative access fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel export config/fixtures/web-automation-positive.ncl >/dev/null
            nickel export config/fixtures/web-automation-tag-positive.ncl >/dev/null
            nickel export config/fixtures/web-automation-untrusted-positive.ncl >/dev/null
            nickel export config/fixtures/web-cobs-positive.ncl >/dev/null
            nickel typecheck config/web-work.ncl
            nickel export config/fixtures/web-work-positive.ncl >/dev/null
            for fixture in config/fixtures/negative-web-work-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative work fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel typecheck config/web-code-review.ncl
            nickel export config/fixtures/web-code-review-positive.ncl >/dev/null
            nickel export config/fixtures/web-code-review-delivery-positive.ncl >/dev/null
            nickel export config/fixtures/web-code-review-intake-positive.ncl >/dev/null
            nickel export config/fixtures/web-code-review-jujutsu-positive.ncl >/dev/null
            nickel export config/fixtures/web-code-review-speculative-positive.ncl >/dev/null
            nickel export config/fixtures/web-code-review-noise-positive.ncl >/dev/null
            for fixture in config/fixtures/negative-web-code-review-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative code review fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel typecheck config/web-sources.ncl
            nickel export config/fixtures/web-sources-positive.ncl >/dev/null
            for fixture in config/fixtures/negative-web-sources-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative source fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel export config/fixtures/web-systemd-positive.ncl >/dev/null
            nickel export deployment/fixtures/positive.ncl >/dev/null
            for fixture in deployment/fixtures/negative-*.ncl; do
              case "$fixture" in *negative-module-*) continue ;; esac
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative deployment fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel export config/fixtures/web-service-positive.ncl >/dev/null
            nickel export config/fixtures/web-service-feeds-positive.ncl >/dev/null
            nickel export config/fixtures/web-service-monorepo-positive.ncl >/dev/null
            nickel export config/fixtures/web-service-untrusted-positive.ncl >/dev/null
            nickel export config/fixtures/web-service-retry-positive.ncl >/dev/null
            nickel export config/fixtures/web-service-patchgate.ncl >/dev/null
            for fixture in config/fixtures/negative-web-service-*.ncl config/fixtures/web-service-pause-negative.ncl config/fixtures/web-service-resume-negative.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative web service fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel export config/fixtures/patchgate-review-positive.ncl >/dev/null
            nickel export config/mantle-build-profile.ncl >/dev/null
            nickel export config/nix-eval-jobs-profile.ncl >/dev/null
            nickel export --field profile config/nix-eval-jobs-onix-profile.ncl >/dev/null
            nickel typecheck config/onixos-ci-pilot-inputs.ncl
            nickel typecheck config/onixos-ci-pilot.ncl
            nickel typecheck config/radicle-ci.ncl
            nickel typecheck config/radicle-ci-worker.ncl
            nickel export config/fixtures/radicle-ci-positive.ncl >/dev/null
            nickel format --check config/radicle-ci.ncl config/radicle-ci-worker.ncl config/fixtures/*radicle-ci*.ncl
            nickel export config/observation-profile.ncl >/dev/null
            nickel export --format json config/observation-profile.ncl > observation-profile.json
            cmp observation-profile.json fixtures/observation/profile-v1.json
            nickel export config/dashboard-profile.ncl >/dev/null
            nickel export --format json config/dashboard-profile.ncl > dashboard-profile.json
            cmp dashboard-profile.json fixtures/dashboard/profile-v1.json
            nickel export config/http-observation-profile.ncl >/dev/null
            nickel export --format json config/http-observation-profile.ncl > http-observation-profile.json
            cmp http-observation-profile.json fixtures/http-observation/profile-v1.json
            nickel export config/fixtures/ucan-observation-positive.ncl >/dev/null
            nickel export --format json config/fixtures/ucan-observation-positive.ncl > ucan-observation-profile.json
            cmp ucan-observation-profile.json fixtures/http-observation/ucan-profile-v1.json
            nickel export config/iroh-observation-profile.ncl >/dev/null
            nickel export --format json config/iroh-observation-profile.ncl > iroh-observation-profile.json
            cmp iroh-observation-profile.json fixtures/iroh-observation/profile-v1.json
            nickel export config/nix-build-profile.ncl >/dev/null
            nickel export config/radicle-workflow-shell.ncl >/dev/null
            nickel typecheck config/iroh-workflow.ncl
            nickel export config/fixtures/iroh-workflow-positive.ncl >/dev/null
            nickel format --check config/iroh-workflow.ncl config/fixtures/*iroh-workflow*.ncl
            nickel typecheck config/github-contracts.ncl
            nickel typecheck config/github-ci.ncl
            nickel typecheck config/github-job.ncl
            # r[impl kiln.implementation.nickel_config] r[verify kiln.implementation.nickel_config.scenario.export] r[verify kiln.implementation.nickel_config.scenario.reject]
            nickel typecheck config/implementer-policy.ncl
            nickel typecheck config/implementer-enrollment.ncl
            nickel export config/fixtures/implementer-policy-positive.ncl >/dev/null
            nickel export config/fixtures/implementer-enrollment-positive.ncl >/dev/null
            for fixture in config/fixtures/negative-implementer-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative implementer policy fixture passed: $fixture" >&2
                exit 1
              fi
            done
            # r[impl kiln.implementation.lattice_cli.scenario.config] r[verify kiln.implementation.lattice_cli.scenario.config]
            nickel typecheck config/implementation-worker.ncl
            nickel export config/fixtures/implementation-worker-positive.ncl >/dev/null
            if nickel export config/fixtures/negative-implementation-worker-transport-v1.ncl >/dev/null 2>&1; then
              echo "negative implementation worker transport fixture passed" >&2
              exit 1
            fi
            nickel typecheck config/gitea-contracts.ncl
            nickel typecheck config/gitea-ci.ncl
            nickel typecheck config/gitea-job.ncl
            nickel export --format json fixtures/gitea/service-profile.ncl >/dev/null
            nickel export --format json fixtures/gitea/job-profile.ncl >/dev/null
            nickel format --check config/gitea-contracts.ncl config/gitea-ci.ncl config/gitea-job.ncl fixtures/gitea/*.ncl
            nickel export --format json fixtures/github/service-profile.ncl >/dev/null
            nickel export --format json fixtures/github/job-profile.ncl >/dev/null
            nickel typecheck config/tangled-contracts.ncl
            nickel typecheck config/tangled-spindle.ncl
            nickel format --check config/tangled-contracts.ncl config/tangled-spindle.ncl config/fixtures/*tangled*.ncl
            nickel export --format json config/fixtures/tangled-spindle-positive.ncl > tangled-spindle-profile.json
            cmp tangled-spindle-profile.json fixtures/tangled/spindle-profile.json
            for fixture in config/fixtures/negative-tangled-*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative Tangled fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel export config/radicle-source-profile.ncl >/dev/null
            nickel export config/radicle-status-profile.ncl >/dev/null
            nickel export config/radicle-mutation-profile.ncl >/dev/null
            nickel export config/stack-profiles.ncl >/dev/null
            nickel export config/cdevents-profile.ncl >/dev/null
            nickel export config/opentelemetry-profile.ncl >/dev/null
            nickel export config/valence-profile.ncl >/dev/null
            nickel export config/webhook-profile.ncl >/dev/null
            nickel export config/sqlite-profile.ncl >/dev/null
            nickel export config/aspen-runtime-profile.ncl >/dev/null
            nickel export config/celld-profile.ncl >/dev/null
            nickel export config/celld-wrangler.ncl >/dev/null
            nickel export config/fixtures/celld-positive.ncl >/dev/null
            nickel export config/fixtures/aspen-runtime-positive.ncl >/dev/null
            nickel export config/fixtures/git-source-positive.ncl >/dev/null
            nickel export config/fixtures/pijul-source-positive.ncl >/dev/null
            nickel export config/fixtures/lattice-workflow-positive.ncl >/dev/null
            nickel export config/fixtures/cairn-gate-positive.ncl >/dev/null
            nickel export config/fixtures/mantle-build-positive.ncl >/dev/null
            nickel export config/fixtures/nix-eval-jobs-positive.ncl >/dev/null
            nickel export config/fixtures/observation-positive.ncl >/dev/null
            nickel export config/fixtures/dashboard-positive.ncl >/dev/null
            nickel typecheck config/dashboard-catalog.ncl
            nickel typecheck config/dashboard-publication.ncl
            nickel typecheck config/dashboard-refresh.ncl
            nickel export config/dashboard-kiln.ncl >/dev/null
            nickel export config/fixtures/dashboard-catalog-positive.ncl >/dev/null
            nickel export config/fixtures/dashboard-publication-positive.ncl >/dev/null
            nickel export config/fixtures/dashboard-refresh-positive.ncl >/dev/null
            nickel export config/fixtures/http-observation-positive.ncl >/dev/null
            nickel export config/fixtures/iroh-observation-positive.ncl >/dev/null
            nickel export config/fixtures/nix-build-positive.ncl >/dev/null
            nickel export config/fixtures/radicle-workflow-positive.ncl >/dev/null
            nickel export config/fixtures/radicle-source-positive.ncl >/dev/null
            nickel export config/fixtures/radicle-status-positive.ncl >/dev/null
            nickel export config/fixtures/radicle-mutation-positive.ncl >/dev/null
            nickel export config/fixtures/radicle-nix-provider-positive.ncl >/dev/null
            nickel export config/fixtures/cairn-cob-external.ncl >/dev/null
            nickel export fixtures/radicle-source-contract/positive/receipt.ncl >/dev/null
            # r[verify kiln.radicle.canonical_source.evidence.scenario.links]
            nickel export fixtures/radicle-source-contract/positive/canonical-receipt.ncl >/dev/null
            for fixture in fixtures/radicle-source-contract/negative/*.ncl; do
              if nickel export "$fixture" >/dev/null 2>&1; then
                echo "negative Radicle source contract fixture passed: $fixture" >&2
                exit 1
              fi
            done
            nickel export fixtures/lattice/workflow-contract/70496e67c7fd4a8b05914161a8e09de2759bebc8/source.ncl >/dev/null
            nickel export fixtures/cairn/gate-contract/15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/source.ncl >/dev/null
            nickel export fixtures/cairn/gate-machine/15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/source.ncl >/dev/null
            nickel export fixtures/mantle/build-contract/d75158b00b080535f2ffea12430665ca3d57ea82/source.ncl >/dev/null
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
          dashboard-publication =
            pkgs.runCommand "kiln-dashboard-publication" { nativeBuildInputs = [ pkgs.python3 ]; }
              ''
                ${copySource}
                export KILN_DASHBOARD_EXECUTABLE=${package}/bin/kiln-dashboard
                python3 -m unittest discover -s scripts -p test_dashboard_publish.py
                python3 -m unittest discover -s scripts -p test_dashboard_refresh.py
                touch "$out"
              '';
          live-console-ui = pkgs.runCommand "kiln-live-console-ui" { nativeBuildInputs = [ pkgs.nodejs ]; } ''
            ${copySource}
            node --test tests/live-console-ui.test.mjs tests/work-views.test.mjs tests/kiln-identity.test.mjs tests/console-inventory.test.mjs
            touch "$out"
          '';
          # Property-based terminal rail for the operator dashboard. The
          # tool is separately executed and never linked into a Kiln package.
          # r[impl kiln.observe.dashboard.property_rail.cohort]
          # r[impl kiln.observe.dashboard.property_rail.cohort.scenario.pin]
          bombadil =
            pkgs.runCommand "kiln-bombadil" { nativeBuildInputs = [ (bombadilPackageFor system) ]; }
              ''
                ${copySource}
                export HOME="$PWD/home"
                mkdir -p "$HOME" traces
                cp checks/bombadil/journal-4-runs.json checks/bombadil/journal.json
                bombadil terminal test \
                  --specification checks/bombadil/kiln-dashboard.spec.ts \
                  --rows 200 --columns 160 --time-limit 60s \
                  --output-path "$PWD/traces/admitted-4-runs" --output-path-overwrite \
                  -- ${packageFor system}/bin/kiln-dashboard \
                  --journal checks/bombadil/journal.json \
                  --profile fixtures/dashboard/profile-v1.json
                cp checks/bombadil/journal-20-runs.json checks/bombadil/journal.json
                bombadil terminal test \
                  --specification checks/bombadil/kiln-dashboard.spec.ts \
                  --rows 200 --columns 160 --time-limit 60s \
                  --output-path "$PWD/traces/bounded-20-runs" --output-path-overwrite \
                  -- ${packageFor system}/bin/kiln-dashboard \
                  --journal checks/bombadil/journal.json \
                  --profile fixtures/dashboard/profile-v1.json
                cp checks/bombadil/journal-pilot.json checks/bombadil/journal.json
                bombadil terminal test \
                  --specification checks/bombadil/kiln-dashboard.spec.ts \
                  --rows 200 --columns 160 --time-limit 60s \
                  --output-path "$PWD/traces/admitted-pilot-journal" --output-path-overwrite \
                  -- ${packageFor system}/bin/kiln-dashboard \
                  --journal checks/bombadil/journal.json \
                  --profile fixtures/dashboard/profile-v1.json
                cp checks/bombadil/journal-tampered.json checks/bombadil/journal.json
                bombadil terminal test \
                  --specification checks/bombadil/kiln-dashboard-rejected.spec.ts \
                  --rows 200 --columns 160 --time-limit 60s \
                  --output-path "$PWD/traces/rejected-tampered" --output-path-overwrite \
                  -- ${packageFor system}/bin/kiln-dashboard \
                  --journal checks/bombadil/journal.json \
                  --profile fixtures/dashboard/profile-v1.json
                rm checks/bombadil/journal.json
                bombadil terminal test \
                  --specification checks/bombadil/kiln-cli-demo.spec.ts \
                  --rows 200 --columns 160 --time-limit 60s \
                  --output-path "$PWD/traces/kiln-cli-demo" --output-path-overwrite \
                  -- ${packageFor system}/bin/kiln demo
                bombadil terminal test \
                  --specification checks/bombadil/kiln-cli-evidence.spec.ts \
                  --rows 200 --columns 160 --time-limit 60s \
                  --output-path "$PWD/traces/kiln-cli-evidence" --output-path-overwrite \
                  -- ${packageFor system}/bin/kiln evidence \
                  checks/bombadil/evidence-input.json \
                  "$PWD/kiln-cli-evidence.json"
                bombadil terminal test \
                  --specification checks/bombadil/kiln-cli-evidence-rejected.spec.ts \
                  --rows 200 --columns 160 --time-limit 60s \
                  --output-path "$PWD/traces/kiln-cli-evidence-rejected" --output-path-overwrite \
                  -- ${packageFor system}/bin/kiln evidence \
                  checks/bombadil/evidence-malformed.json \
                  "$PWD/kiln-cli-evidence-rejected.json"
                cp -r traces "$out"
              '';
          # The imported contract library moves with Cairn; the reviewed gate
          # executable cohort stays at 15f00875, so its wire fixtures and
          # contract must remain byte-identical in the newer import.
          cairn-gate-cohort = pkgs.runCommand "kiln-cairn-gate-cohort" { } ''
            test '${cairnGateRevision}' = '317d3ff648f01a5ea1572e6126f4dd71bc7afa99'
            test '${cairn.rev}' = '${cairnGateRevision}'
            cmp ${cairnGateSource}/fixtures/cairn-gate-contract/producer/fixtures-v1.json \
              ${self}/fixtures/cairn/gate-contract/15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/producer-fixtures-v1.json
            cmp ${cairnGateSource}/contracts/cairn-gate-contract.ncl \
              ${self}/fixtures/cairn/gate-contract/15f00875562025e7ea7e0d1f4af24d1a2e2ac06f/contract.ncl
            touch "$out"
          '';
          cairn-policy =
            pkgs.runCommand "kiln-cairn-policy"
              {
                nativeBuildInputs = [
                  (cairnPackageFor system)
                  pkgs.jq
                  pkgs.nickel
                  pkgs.python3
                ];
              }
              ''
                ${copySource}
                CAIRN_POLICY=/nonexistent/lattice-default.json sh tests/cairn-workflow-policy.sh
                bash tests/import-pinned-source-pins.sh
                touch "$out"
              '';
          mantle-build-cohort =
            pkgs.runCommand "kiln-mantle-build-cohort"
              {
                nativeBuildInputs = [
                  pkgs.b3sum
                  pkgs.jq
                  pkgs.nickel
                ];
              }
              ''
                test '${mantleBuildRevision}' = 'd75158b00b080535f2ffea12430665ca3d57ea82'
                nickel export ${self}/fixtures/mantle/build-contract/d75158b00b080535f2ffea12430665ca3d57ea82/source.ncl \
                  | jq -e --arg revision '${mantleBuildRevision}' '.revision == $revision' >/dev/null
                nickel export ${self}/config/mantle-build-profile.ncl \
                  | jq -e --arg revision '${mantleBuildRevision}' '.profile.revision == $revision' >/dev/null
                nickel export ${self}/config/stack-profiles.ncl \
                  | jq -e --arg revision '${mantleBuildRevision}' \
                    'any(.. | objects; (.id? == "mantle-build-v1") and (.source_revision == $revision))' >/dev/null
                cmp ${mantleBuildSource}/fixtures/mantle-build-contract/producer/fixtures-v1.json \
                  ${self}/fixtures/mantle/build-contract/d75158b00b080535f2ffea12430665ca3d57ea82/producer-fixtures-v1.json
                cmp ${mantleBuildSource}/contracts/mantle-build-contract.ncl \
                  ${self}/fixtures/mantle/build-contract/d75158b00b080535f2ffea12430665ca3d57ea82/contract.ncl
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
          # r[impl kiln.web.http_contract.drift]
          http-contract-drift =
            let
              tool = rustPlatform.buildRustPackage {
                pname = "kiln-http-contract-drift";
                version = "0.1.0";
                src = ./tools/http-contract-drift;
                cargoLock = {
                  lockFile = ./tools/http-contract-drift/Cargo.lock;
                  allowBuiltinFetchGit = true;
                };
                doCheck = true;
              };
            in
            pkgs.runCommand "kiln-http-contract-drift-check"
              {
                nativeBuildInputs = [ tool ];
              }
              ''
                set -eu
                contract=${./contracts/http-api}
                kiln-http-contract-drift "$contract/baseline/openapi.json" "$contract/openapi.json" > baseline.out
                grep -q '^decision=pass version_changed=false$' baseline.out
                for fixture in trivial versioned-incompatible; do
                  kiln-http-contract-drift "$contract/fixtures/$fixture/baseline.json" "$contract/fixtures/$fixture/current.json" > "$fixture.out"
                  if test "$fixture" = trivial; then
                    grep -q '^Trivial ' "$fixture.out"
                    grep -q '^decision=pass version_changed=false$' "$fixture.out"
                  else
                    grep -q -E '^(BackwardIncompatible|ForwardIncompatible|Incompatible) ' "$fixture.out"
                    grep -q '^decision=pass version_changed=true$' "$fixture.out"
                  fi
                done
                for fixture in incompatible unhandled; do
                  if kiln-http-contract-drift "$contract/fixtures/$fixture/baseline.json" "$contract/fixtures/$fixture/current.json" > "$fixture.out"; then
                    status=0
                  else
                    status=$?
                  fi
                  test "$status" -eq 1
                  grep -q '^decision=fail version_changed=false$' "$fixture.out"
                  if test "$fixture" = unhandled; then
                    grep -q '^Unhandled ' "$fixture.out"
                  else
                    grep -q -E '^(BackwardIncompatible|ForwardIncompatible|Incompatible) ' "$fixture.out"
                  fi
                done
                touch "$out"
              '';
          # r[impl kiln.radicle.mutation_conformance]
          radicle-mutation-native-fixtures =
            if system == "x86_64-linux" then
              radiclePatchRustPlatform.buildRustPackage {
                pname = "kiln-radicle-mutation-native-fixtures";
                version = "0.1.0";
                src = ./tools/radicle-patch;
                cargoLock = {
                  lockFile = ./tools/radicle-patch/Cargo.lock;
                };
                nativeBuildInputs = [
                  pkgs.cmake
                  pkgs.git
                  pkgs.pkg-config
                ];
                buildInputs = [
                  pkgs.openssl
                  pkgs.zlib
                ];
                buildPhase = ''
                  runHook preBuild
                  cargo test --locked --offline
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
              cargo tree -p onix-kiln-core --edges normal > core-tree.txt
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
          # r[related kiln.toolchain.pin.scenario.shell]
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          browserToolchain = pkgs.rust-bin.fromRustupToolchainFile ./web/dashboard-index/rust-toolchain.toml;
        in
        {
          dashboard-index = pkgs.mkShell {
            packages = [
              browserToolchain
              pkgs.python3
              pkgs.wasm-bindgen-cli
              pkgs.esbuild
              pkgs.b3sum
            ];
          };
          web-testing = pkgs.mkShell {
            packages = [
              pkgs.python3
              pkgs.chromium
              (bombadilPackageFor system)
              (websiteExplorationPackageFor system)
            ];
            CHROME = "${pkgs.chromium}/bin/chromium";
          };
          default =
            let
              kilnCairn = cairnPackageFor system;
            in
            pkgs.mkShell {
              packages = [
                kilnCairn
                toolchain
                pkgs.b3sum
                pkgs.bubblewrap
                pkgs.cmake
                pkgs.nickel
                pkgs.nixfmt
                pkgs.pkg-config
                pkgs.python3
                pkgs.watchexec
                pkgs.zlib
                pkgs.openssl
              ];
              shellHook = ''
                resolved_cairn=$(command -v cairn || true)
                if [ "$resolved_cairn" != "${kilnCairn}/bin/cairn" ]; then
                  printf 'warning: cairn resolves to %s instead of Kiln pinned %s\n' \
                    "$resolved_cairn" "${kilnCairn}/bin/cairn" >&2
                fi
              '';
            };
        }
      );
    };
}
