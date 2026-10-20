## Why

The shell already exposes deterministic Web-of-Trust principal digest derivation, but caller-supplied digest bytes for WOT evidence still cross a small shell-local length/admission boundary before becoming vendored `PrincipalId` values. The vendored `verified-logic` snapshot now has an exact finite primitive for admitting raw principal digest bytes, so the public shell should route this predicate through the verified bridge too.

## What Changes

- Add a data-only helper for checking whether caller-supplied Web-of-Trust principal digest bytes are full-width according to vendored verified-logic semantics.
- Route internal WOT request conversion from raw digest bytes through the vendored principal digest admission constructor instead of manual struct construction.
- Document that the helper validates only digest shape and does not resolve identities, validate trust evidence, authorize UCANs, perform revocation freshness checks, access storage/network, sign, or read clocks.

## Capabilities

### New Capabilities
- `library.verified-wot-principal-digest-admission`: expose raw WOT principal digest admission through vendored verified-logic.

## Impact

- **Files**: `src/verified.rs`, `src/lib.rs`, `scripts/check-ucan-bridge.sh`, `README.md`, `docs/public-api.md`.
- **APIs**: add `web_of_trust_principal_digest_is_valid(&[u8]) -> bool`.
- **Dependencies**: none.
- **Testing**: focused digest-admission tests, full Rust checks, bridge/no-std guards, OpenSpec validation, diff/Nix gates.
