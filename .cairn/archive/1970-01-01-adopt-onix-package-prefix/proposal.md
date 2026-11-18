## Why

crates.io already carries unrelated crates named `kiln-core` and `kiln-cli`, both from `github.com/nebucloud/kiln`, and an unrelated crate named `kiln`. A reader who runs `cargo install kiln-cli` installs a different project, and Kiln could never publish those two packages under their current names. The operator chose the stack's `onix-` package prefix to resolve the collision.

## What Changes

- Rename package `kiln-core` to `onix-kiln-core` and package `kiln-cli` to `onix-kiln-cli`. The library name `kiln_core`, the binary name `kiln`, both directories and every Rust path stay unchanged.
- Declare the workspace dependency as `kiln-core = { package = "onix-kiln-core", ... }`, so member manifests and source need no edit.
- Update the two Octet check templates and their lock files, the root lock file, the flake's package selectors, the celld boundary script and the documented `cargo -p` commands.
- State the package names in the architecture document, the README and the launch kit, and record the note for consumers that pin Kiln by Git revision.

The other 52 first-party package names in tracked manifests outside `vendor/` were not found on crates.io and keep their names.

## Impact

- **Current consumer:** the operator preparing the `0.1.0` launch. Cairn, OnixOS and the Kiln Aspen host pin `kiln-core` at earlier Git revisions and are unaffected until they move their pins.
- **Durable capability:** the two packages have names Kiln can own on a registry; documented commands select them unambiguously.
- **Owner:** Kiln maintainers.
- **Files**: `Cargo.toml`, `Cargo.lock`, `crates/kiln-core/Cargo.toml`, `crates/kiln-cli/Cargo.toml`, `checks/aspen-runtime-octet/Cargo.{toml,lock}`, `checks/radicle-broker-wire-octet/Cargo.{toml,lock}`, `flake.nix`, `tests/celld-boundaries.sh`, `README.md`, `CHANGELOG.md`, `docs/architecture.md`, `docs/operator.md`, `docs/observation-dashboard.md`, `runtime/kiln-aspen-host/README.md`, `docs/gtm/*.md` and this package.
- **Testing**: confirm package, library and binary names with `cargo metadata`; run the workspace tests and Clippy; check each nested lock file with `cargo metadata --locked`; run the celld boundary script in an isolated copy; search tracked files for stale package selectors. The Nix checks that select these packages are not run here and are named as not exercised.
- **No accepted-spec effect:** package names are build metadata. No requirement, behavior, schema, library path or binary name changes.
