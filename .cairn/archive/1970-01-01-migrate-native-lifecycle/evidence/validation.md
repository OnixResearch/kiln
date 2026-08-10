# Validation evidence

Date: 2026-08-10

## Scope

This evidence covers the Kiln lifecycle-store migration only. It does not prove CI behavior, provider effects, release readiness, or whole-system correctness.

## Baseline

Strict Cairn validation failed before the change with `cairn.lifecycle_directory.migration_required`.

## Migration

- Cairn revision: `695124d459574ba7aeba6097310d237f393c243c`
- Preview plan: `1971dce186eb7847d3678920844196e131a1a608e7c2d632caa354a60eb37202`
- Source manifest: `2d16d705d9cf1243d7239e4b34411da2cb7aa93140a190651e763df0a9fd3164`
- Destination manifest: `77b78bc73e4407efb745ddfb0cd47b799d310df95ed241cd26b5f227c811d070`
- Preview receipt BLAKE3: `e6fa61ae4a7f34ab04bb0d93c861bfd8582718b41c844644b311f4211a9075af`
- Execution receipt BLAKE3: `c58ef967a904857f709fd2802397ff90e7b8ebcfdb2e387b7006f094b4d94881`

A recursive byte comparison passed before removal of `cairn/`.

The negative replay used the old plan after source removal. Cairn rejected it with a plan-hash mismatch before mutation.

## Policy

The local policy snapshots Cairn revision `695124d459574ba7aeba6097310d237f393c243c` and applies a narrow Kiln traceability overlay.

- Overlay BLAKE3: `9df10fef5961837857ca7309b6f5034489e08918968373c14cf1e3adf2978ec8`
- Generated policy BLAKE3: `167912096c477268eb36bc42fc66bed18339a5e0d4f553d65228afaf74378c14`

Policy export was deterministic. Three malformed traceability fixtures failed as required.

## Checks

These checks passed:

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- strict Cairn validation
- proposal, design, and tasks gates
- policy export freshness
- traceability coverage after accepted-spec sync
- `nix flake check path:$PWD -L`

The workspace test run passed 78 tests and all documentation tests. Traceability reported no missing or dangling requirements.

The traceability receipt BLAKE3 was `99dcfbcf1f383226bffa75bb75da8db2bd43b768f837f1f3c8ee5e6aabdbe9f5`.

Nix used the local builder after the configured remote builder was unavailable. All evaluated compatible host checks passed. The incompatible `aarch64-linux` system was omitted.
