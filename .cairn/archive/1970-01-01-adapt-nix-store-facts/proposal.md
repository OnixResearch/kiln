## Why

Kiln's Nix build adapter trusts discovery-provided output names and paths for
an exact `.drv`, records no NAR hash, references, or input closure, and has no
typed sandbox-constraint vocabulary. Snix models these facts directly
(`nix-compat` derivations/PathInfo/NAR, `BuildRequest` inputs and
`refscan_needles`), but its crates are GPL-3.0 and must not be linked into
Kiln's MIT/Apache artifacts. The equivalent permissive crates are available:
`nix-derivation` and `nix-archive` (Apache-2.0).

## What Changes

- Add a pure `kiln-nix-store` crate for bounded derivation admission, NAR
  hashing, reference scanning, input-closure facts, and canonical narinfo
  projection using `nix-derivation`, `nix-archive`, and `nix-narinfo`.
- Extend `kiln-core` build plans and observations with typed constraints,
  optional product store-object facts, and optional input-closure facts.
- Parse and validate the exact `.drv` before dispatch in the Nix build path;
  derive expected output names from it and verify task/discovery bindings.
- Record NAR SHA-256/size, references and reference matches for Nix and Mantle
  product outputs where the runtime can observe them.
- Add a read-only cache projection command that writes a bounded Nix binary
  cache directory from admitted observations and realized output paths.

## Impact

- **Files**: new `crates/kiln-nix-store`, `kiln-core/src/build.rs`,
  `kiln-adapter-nix`, `kiln-nix-build`, `kiln-adapter-mantle`, workspace
  manifests/lock, docs and Cairn evidence.
- **Testing**: pure derivation/NAR/narinfo tests, adapter fixtures for
  mismatches and bounds, cache projection round trips, focused/full Cargo,
  Clippy, rustfmt, Cairn.
- **Non-claims**: facts prove only observed derivations, NAR bytes, references
  and projection consistency. They do not prove build correctness, hermeticity,
  cache truth, reproducibility, source trust or release readiness.
