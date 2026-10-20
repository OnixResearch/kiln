## Why

`vendor/verified-logic` includes a finite invocation receipt checker that validates executor/receipt ordering semantics, but the `ucan` shell only exposes invocation authorization helpers. Consumers that want deterministic receipt preflight would otherwise duplicate those semantics locally.

## What Changes

- Add a small public verified bridge helper for receipt execution checks.
- Document that the helper only checks executor equality and receipt timestamp ordering; it does not create receipt storage, persistence, replay protection, or network behavior.
- Add focused tests and bridge-guard coverage.

## Impact

- Files: `src/verified.rs`, `src/lib.rs`, docs, bridge guard, OpenSpec library spec.
- Verification: focused receipt bridge test plus the standard UCAN cargo/OpenSpec/Nix gates.
