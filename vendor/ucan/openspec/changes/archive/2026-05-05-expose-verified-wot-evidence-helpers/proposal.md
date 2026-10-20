## Why

The UCAN shell already exposes an aggregate Web-of-Trust UCAN authorizer, but callers that prepare or audit finite WOT evidence still cannot ask the shell to run the vendored verified attestation and revocation evidence predicates directly. The latest audit identified these exact data-only helper seams as the best small bridge candidate.

## What Changes

- Add public verified bridge helpers for finite WOT attestation evidence checks.
- Add public verified bridge helpers for finite WOT revocation evidence checks.
- Optionally expose the vendored finite trust-threshold predicate when it improves caller evidence preparation.
- Document the helpers as advanced data-only predicates, not as DID resolution, proof fetching, cryptographic key management, registry freshness, or shell authorization replacements.

## Capabilities

### New Capabilities
- `library.verified-wot-evidence-helpers`: Callers can preflight finite WOT evidence through vendored verified-logic predicates before invoking aggregate authorization.

### Modified Capabilities
- `library.verified-wot-ucan-authorizer`: Documentation can point to lower-level evidence helpers for callers that build WOT inputs explicitly.

## Impact

- **Files**: `src/verified.rs`, `src/lib.rs`, README/API docs, bridge guard script, OpenSpec library spec.
- **APIs**: New public helper functions and, if needed, public wrapper/result types for finite WOT evidence helper outputs.
- **Dependencies**: No new runtime/network dependencies.
- **Testing**: Focused positive/negative WOT evidence helper tests, bridge guard, full Rust/OpenSpec/Nix gates.
