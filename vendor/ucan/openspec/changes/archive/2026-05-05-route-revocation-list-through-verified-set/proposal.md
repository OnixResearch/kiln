## Why

`RevocationList` still performs shell-local byte-vector membership over proof references. The vendored `verified-logic` snapshot already exposes a verified finite revocation-set lookup for caller-supplied CID/reference bytes, so the in-memory helper should reuse that bridge instead of carrying a parallel membership scan.

## What Changes

- Add a small std-shell bridge helper that maps proof-reference bytes into `verified_logic::ucan_revocation::RevocationSet` and calls its verified lookup.
- Route `RevocationList::is_revoked` through that helper while preserving the caller-injected revocation trait boundary and fail-closed backend behavior.
- Strengthen bridge guards and public docs so local membership scans do not creep back into `RevocationList`.

## Impact

- Files: `src/verified.rs`, `src/token.rs`, `src/lib.rs`, bridge guard, docs, canonical library spec.
- APIs: additive public bridge helper only; no network, registry, persistence, or global revocation behavior.
- Testing: focused revocation bridge test plus full cargo/Nix/OpenSpec gates.
