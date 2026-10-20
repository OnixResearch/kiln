## Why

Recursive proof verification already enforces an explicit depth limit in the std shell, but the admission predicate is still a shell-local numeric comparison. The vendored `verified-logic` snapshot contains a UCAN delegation-depth primitive that defines depth as `chain_len - 1` and checks budget admission.

## What Changes

- Add a public `proof_depth_within_budget` bridge helper around the verified delegation-depth checker.
- Route proof-depth enforcement through that helper while preserving the existing structured limit error and traversal order.
- Document and guard the bridge so future proof-depth changes do not reintroduce a parallel shell-local predicate.

## Impact

- Files: `src/verified.rs`, `src/token.rs`, `src/lib.rs`, docs, guard script, OpenSpec.
- APIs: Adds a narrow public verified bridge helper; no resolver, network, storage, or core dependency changes.
- Verification: Focused depth tests plus full cargo/clippy/no-std/bridge/OpenSpec/Nix gates.
