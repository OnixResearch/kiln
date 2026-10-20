# Verified-logic Josh sync pilot

`vendor/verified-logic` is the UCAN-facing subtree snapshot of the Trellis `verified-logic` crate. Trellis remains authoritative; UCAN keeps an in-tree snapshot so Cargo and Nix builds do not require a sibling checkout.

## Josh filter

The pilot filter lives at `josh/verified-logic.josh` and selects the same paths as the existing importer:

- `Cargo.toml`
- `README.md` when present
- `build.rs` when present
- `.cargo/` when present
- `src/`

That filter is the canonical path selection for future Josh/Josh-sync automation. The current `rust-lang/josh-sync` binary is rustc-parent-specific, so this repo keeps the filter and verifier in-tree while preserving the existing copy importer.

## Refresh workflow

Refresh from the committed Trellis source checkout:

```sh
./scripts/sync-verified-logic.sh ../trellis
```

The importer archives the selected paths from the source repo's committed `HEAD`, ignores uncommitted sibling changes, and records the imported commit in `vendor/verified-logic-revision.txt`.

Verify the checked-in vendor tree matches the recorded Trellis revision:

```sh
rustc scripts/check-verified-logic-josh-sync.rs -o /tmp/check-verified-logic-josh-sync
/tmp/check-verified-logic-josh-sync --source-repo ../trellis
```

Use `--config-only` when a sandbox cannot access the Trellis checkout and only needs to validate the Josh filter, verifier unit tests through the Nix check, and revision metadata.

## Pre-commit

`.pre-commit-config.yaml` runs `nix build .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).verified-logic-josh-sync-config --no-link`. That hook is intentionally local-only so commits do not require a sibling Trellis checkout; run the full verifier manually before claiming byte-for-byte parity with an upstream Trellis revision.

## Graduation criteria

Keep this as a filter-plus-verifier pilot until UCAN/Trellis changes repeatedly need bidirectional atomic review. Graduate to a dedicated generic josh-sync wrapper when the workflow needs to:

1. pull UCAN-side vendor edits back into Trellis history;
2. push Trellis changes into `vendor/verified-logic` with conflict handling;
3. produce sync PR branches automatically; and
4. preserve this filter as the single source of truth for selected paths.
