{
  pkgs,
  unikraft,
  app-elfloader,
  lib-libelf,
}:
let
  version = unikraft.rev;
  sourcePins = pkgs.writeText "unikernel-source-pins.json" (
    builtins.toJSON {
      schema = "kiln.unikernel.sources.v1";
      sources =
        map
          (source: {
            revision = source.rev;
            nar_hash = source.narHash;
          })
          [
            unikraft
            app-elfloader
            lib-libelf
          ];
    }
  );
  guest = pkgs.stdenvNoCC.mkDerivation {
    pname = "kiln-unikraft-firecracker";
    inherit version;
    dontUnpack = true;
    nativeBuildInputs = with pkgs; [
      gcc14
      gnumake
      flex
      bison
      m4
      python3
      binutils
      b3sum
      jq
      which
      inetutils
    ];
    # Unikraft controls its freestanding ABI and link flags, not the Nix cc wrapper.
    hardeningDisable = [ "all" ];
    buildPhase = ''
      runHook preBuild
      cp -r ${unikraft} core
      cp -r ${app-elfloader} app
      mkdir -p libs
      cp -r ${lib-libelf} libs/libelf
      chmod -R u+w core app libs
      patch -d core -p1 < ${./native-lifecycle.patch}
      # Upstream invokes support scripts directly, including /usr/bin/env
      # interpreters unavailable inside the Nix sandbox.
      chmod u+x core/support/build/*.sh core/support/scripts/configupdate
      patchShebangs --build core app libs
      export SOURCE_DATE_EPOCH=1
      make -C core A="$PWD/app" L="$PWD/libs/libelf" O="$PWD/build" \
        CC=gcc LD=gcc OBJCOPY=objcopy OBJDUMP=objdump STRIP=strip \
        HOSTUTC=1970-01-01T00:00:01+00:00 HOSTNAME=kiln HOSTUSER=builder \
        defconfig UK_DEFCONFIG=${./uk.defconfig}
      make -C core A="$PWD/app" L="$PWD/libs/libelf" O="$PWD/build" \
        CC=gcc LD=gcc OBJCOPY=objcopy OBJDUMP=objdump STRIP=strip \
        HOSTUTC=1970-01-01T00:00:01+00:00 HOSTNAME=kiln HOSTUSER=builder \
        UK_CFLAGS="-ffile-prefix-map=$NIX_BUILD_TOP=/build" \
        UK_ASFLAGS="-ffile-prefix-map=$NIX_BUILD_TOP=/build" -j"$NIX_BUILD_CORES"
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out"
      cp build/elfloader_fc-x86_64 "$out/kernel"
      cp app/.config "$out/config"
      substituteInPlace "$out/config" --replace-fail "$NIX_BUILD_TOP" /build
      cp ${sourcePins} "$out/sources.json"
      # Retain complete immutable source/license headers in the output closure:
      # core and imported libraries have per-file terms, not one blanket grant.
      mkdir -p "$out/share/unikernel-sources"
      ln -s ${unikraft} "$out/share/unikernel-sources/core"
      ln -s ${app-elfloader} "$out/share/unikernel-sources/elfloader"
      ln -s ${lib-libelf} "$out/share/unikernel-sources/libelf"
      # Identity of the exact source archives, independently of NAR transport.
      for pair in 'core:${unikraft}' 'app:${app-elfloader}' 'libelf:${lib-libelf}'; do
        name="''${pair%%:*}"
        source="''${pair#*:}"
        tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner \
          --format=gnu -C "$source" -cf "$name.tar" .
        hash=$(b3sum --no-names "$name.tar")
        jq --arg name "$name" --arg hash "$hash" \
          '.archive_blake3[$name] = $hash' "$out/sources.json" > sources.next.json
        mv sources.next.json "$out/sources.json"
      done
      patch_hash=$(b3sum --no-names ${./native-lifecycle.patch})
      config_hash=$(b3sum --no-names ${./uk.defconfig})
      jq --arg patch "$patch_hash" --arg config "$config_hash" \
        '.lifecycle_patch_blake3 = $patch | .defconfig_blake3 = $config' \
        "$out/sources.json" > sources.next.json
      mv sources.next.json "$out/sources.json"
      vmm_hash=$(b3sum --no-names ${pkgs.firecracker}/bin/firecracker)
      hash=$(b3sum --no-names "$out/kernel")
      jq -n --arg version '${version}' --arg kernel "$out/kernel" --arg hash "$hash" \
        --arg vmm '${pkgs.firecracker}/bin/firecracker' --arg vmm_hash "$vmm_hash" \
        --arg vmm_version '${pkgs.firecracker.version}' \
        --slurpfile sources "$out/sources.json" \
        '{schema:"kiln.unikernel.guest.v1",guest_kind:"unikernel",project:"unikraft",version:$version,kernel:{path:$kernel,blake3:$hash},firecracker:{path:$vmm,blake3:$vmm_hash,version:$vmm_version},sources:$sources[0],hostile_isolation_claim:false}' \
        > "$out/manifest.json"
      runHook postInstall
    '';
    dontFixup = true;
    meta = {
      description = "Native Unikraft ELF loader kernel for trusted static PIE tests on Firecracker";
      license = pkgs.lib.licenses.bsd3;
      platforms = [ "x86_64-linux" ];
    };
  };
  toolchain = pkgs.rust-bin.stable."1.89.0".default.override {
    targets = [ "x86_64-unknown-linux-musl" ];
  };
  rustFixture =
    name: source:
    pkgs.runCommand name
      {
        nativeBuildInputs = [ toolchain ];
      }
      ''
        mkdir -p "$out/bin"
        rustc --edition=2021 --target=x86_64-unknown-linux-musl \
          -C relocation-model=pic -C debuginfo=0 --remap-path-prefix "$NIX_BUILD_TOP"=/build \
          ${source} -o "$out/bin/test"
      '';
  rustBinary = rustFixture "kiln-unikernel-static-rust" ./test.rs;
  probeBinary = rustFixture "kiln-unikernel-network-probe" ./network-probe.rs;
  goBinary =
    pkgs.runCommand "kiln-unikernel-static-go"
      {
        nativeBuildInputs = [
          pkgs.go
          pkgs.gcc14
        ];
        LIBRARY_PATH = "${pkgs.glibc.static}/lib";
        CGO_ENABLED = "1";
      }
      ''
        export HOME="$TMPDIR" GOCACHE="$TMPDIR/go-cache" GOPATH="$TMPDIR/go"
        export GOPROXY=off GOSUMDB=off GOENV=off
        mkdir -p "$out/bin"
        go build -trimpath -buildvcs=false -buildmode=pie \
          -ldflags='-buildid= -linkmode=external -extldflags=-static-pie' \
          -o "$out/bin/test" ${./test.go}
      '';
  mkImage =
    {
      binary,
      fixtures ? [ ],
      name ? "kiln-unikernel-image",
    }:
    let
      fixtureSpec = pkgs.writeText "unikernel-fixtures.json" (builtins.toJSON fixtures);
    in
    pkgs.runCommand name
      {
        nativeBuildInputs = [
          pkgs.python3
          pkgs.b3sum
        ];
        passthru = { inherit guest binary; };
      }
      ''
        python3 ${./image.py} --binary ${pkgs.lib.escapeShellArg (toString binary)} \
          --fixtures ${fixtureSpec} --guest ${guest} --out "$out"
      '';
in
{
  inherit
    guest
    mkImage
    rustBinary
    goBinary
    probeBinary
    ;
  rustImage = mkImage {
    binary = "${rustBinary}/bin/test";
    name = "kiln-unikernel-rust-image";
  };
  goImage = mkImage {
    binary = "${goBinary}/bin/test";
    name = "kiln-unikernel-go-image";
  };
  probeImage = mkImage {
    binary = "${probeBinary}/bin/test";
    name = "kiln-unikernel-probe-image";
  };
}
