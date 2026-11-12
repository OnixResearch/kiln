## Context

Kiln's packages set `publish = false` or are unpublished; distribution is source plus the Nix flake. Of the 54 first-party package names in tracked manifests outside `vendor/`, crates.io holds exactly two under other owners: `kiln-core` and `kiln-cli`. The stack already prefixes selected packages with `onix-`, for example `onix-radicle-cob-git-store` and `onix-cairn-radicle-cob`, while keeping their directory names.

## Decisions

### Decision: Prefix only the colliding packages

**Choice:** Rename `kiln-core` to `onix-kiln-core` and `kiln-cli` to `onix-kiln-cli`. Leave the other 52 package names unchanged.

**Rationale:** The collision exists for these two names. Cairn, OnixOS and the Kiln Aspen host depend on Kiln packages by name through pinned Git revisions; renaming every package would change each of those declarations at the next pin move for no present benefit. A family-wide prefix stays a mechanical follow-up if registry publication is ever prepared.

### Decision: Keep library, binary and directory names

**Choice:** Set `[lib] name = "kiln_core"` in `crates/kiln-core/Cargo.toml` and rename the workspace dependency with `package = "onix-kiln-core"` under the existing key `kiln-core`. The `kiln` binary already has an explicit `[[bin]]` name.

**Rationale:** No Rust source, traceability marker, fixture or documented module path changes. Cargo resolves the key `kiln-core` to the renamed package, so the 43 member manifests that inherit the workspace dependency are untouched.

### Decision: Do not touch pinned consumers

**Choice:** Leave `runtime/kiln-aspen-host`, which pins Kiln at an earlier Git revision, unchanged. Document that a consumer moving its pin past this change declares `kiln-core = { package = "onix-kiln-core", ... }`.

**Rationale:** A pinned revision keeps the package name it was published with. Changing the declaration before the pin moves would break the build.

### Decision: No-spec classification

**Choice:** Use `no-spec-delta` with change-local acceptance and an agentic self-review of the planning inputs.

**Rationale:** Package names are build metadata. The self-review is not independent approval.

## Risks / Trade-offs

- The package name now differs from the directory and library name for two crates. The architecture document states the mapping.
- Nix checks select packages by name. Earlier Nix builds on this host stalled on store contention, so the equivalent Cargo selectors are exercised directly; the evidence names each Nix check that did not complete.
- Both nested lock files were already stale at `main`: `kiln-core` gained a `serde_json` dependency after they were generated, and `cargo metadata --locked` refused them. Regenerating them for the rename also records that edge. No registry package version changes.
- A consumer that moves its pin without changing its declaration fails at dependency resolution with a clear missing-package error; nothing fails silently.
- The crate named `kiln` on crates.io is unrelated to any package here. Product-name clearance remains an operator gate.

## Verification Strategy

Read package, library and binary names from `cargo metadata`. Run the workspace tests and Clippy with `--locked`, so the regenerated root lock file is checked. Materialize each Octet check template with the repository path and run `cargo metadata --locked --offline` against its regenerated lock file. Run `tests/celld-boundaries.sh` from an isolated copy of the tracked tree, including its negative case. Search tracked files outside archived evidence for `-p kiln-core`, `-p kiln-cli` and quoted selectors. Run the planning gates and `cairn validate`.
