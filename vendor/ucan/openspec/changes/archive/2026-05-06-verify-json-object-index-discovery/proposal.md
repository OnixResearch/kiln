## Why

UCAN currently routes decoded JSON object structural admission through vendored `verified-logic`, but the std shell still computes the first and last significant JSON byte indexes locally before invoking that predicate. Those indexes are trusted shell-supplied facts even though they are deterministic byte-level properties that can be discovered by a small executable verified primitive.

Moving JSON significant-byte index discovery into `../verified-logic` reduces live compact-token parsing trust without claiming to verify base64url decoding, serde, canonical JSON, duplicate-key handling, UTF-8/string extraction, signatures, hashing, backends, proof truth, or token provenance.

## What Changes

- Add a verified JSON object envelope/index-discovery primitive in sibling `../verified-logic` and sync the vendored snapshot into UCAN.
- Replace UCAN's local `json_significant_byte_indexes` / `is_json_whitespace_byte` trust seam with a bridge that either returns verified first/last significant indexes for a decoded top-level object envelope or fails closed.
- Keep the public boundary and docs explicit: the new primitive verifies only finite decoded-byte whitespace/object-envelope facts and index discovery.

## Capabilities

### Modified Capabilities

- `library.verified-compact-token-codec-boundary`: decoded JSON object structural admission no longer depends on UCAN-local significant-index discovery.
- `library.verified-bridge-coverage-sweep`: the bridge coverage inventory must classify the new JSON index-discovery bridge as live-path and update the vendored module notes.

## Impact

- **Repositories**: `/home/brittonr/git/verified-logic` first, then `/home/brittonr/git/ucan` vendored sync.
- **Files**: expected UCAN touch points include `vendor/verified-logic*`, `src/verified.rs`, `src/lib.rs` if the public bridge changes, `src/token.rs`, `docs/verified-boundaries.md`, `docs/verified-bridge-coverage.md`, and `scripts/check-ucan-bridge.sh` if guard coverage needs drift detection.
- **APIs**: a new or replacement verified JSON object envelope/index helper may be exposed; existing behavior should remain fail-closed for empty, whitespace-only, array, scalar, or malformed envelope bytes.
- **Testing**: sibling verified-logic tests/Verus checks, UCAN fmt/test/clippy/no-std/bridge/OpenSpec gates, and Nix flake check.
