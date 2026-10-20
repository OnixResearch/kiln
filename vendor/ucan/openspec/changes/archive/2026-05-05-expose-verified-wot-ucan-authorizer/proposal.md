## Why

The vendored verified-logic snapshot includes a Web-of-Trust UCAN authorizer facade that composes UCAN envelope checks with caller-supplied ability/caveat/linkage facts and finite rooted trust resolution. UCAN's std shell currently exposes the lower-level UCAN bridges but not this composed Web-of-Trust admission surface.

## What Changes

- Expose a data-only advanced helper that delegates caller-supplied Web-of-Trust UCAN authorization inputs to `verified_logic::web_of_trust_ucan_authorizer`.
- Add public data wrappers for finite principal, attestation, revocation, caveat, and UCAN payload inputs required by that helper.
- Document that the helper does not replace production token verification, caveat policy evaluation, DID/CID decoding, revocation backend/freshness checks, cryptography, proof fetching, networking, persistence, replay checks, or ambient clock behavior.

## Impact

- Files: `src/verified.rs`, `src/lib.rs`, `README.md`, `docs/public-api.md`, `scripts/check-ucan-bridge.sh`, and OpenSpec library docs.
- Verification: focused Rust tests, bridge guard, full Cargo/Nix gates, strict OpenSpec validation, and whitespace checks.
