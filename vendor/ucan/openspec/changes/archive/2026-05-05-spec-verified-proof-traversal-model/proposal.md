## Why

The largest remaining unverified UCAN shell gap is proof traversal orchestration: recursive proof verification, cycle detection, traversal stack handling, proof-count/depth/input limits, and fail-closed backend propagation. Existing vendored primitives verify individual links and bounds, but not the shell traversal algorithm as a whole.

## What Changes

- Specify the resolved-data model needed before any future verified proof traversal primitive can replace shell-local orchestration checks.
- Define explicit non-goals for resolver/proof-store backends, cryptography, compact token parsing, and revocation registry behavior.
- Require an implementation slice to either add a new vendored verified primitive first or clearly document why only shell tests/guards are possible.

## Capabilities

### New Capabilities
- `library.verified-proof-traversal-model`: A future implementation has a precise model for finite resolved proof traversal evidence and shell boundary preservation.

## Impact

- **Files**: OpenSpec library spec now; future implementation may touch verified-logic vendoring, `src/token.rs`, bridge guards, and tests.
- **APIs**: No immediate API requirement unless a future bridge requires public resolved-chain input wrappers.
- **Dependencies**: No new runtime dependencies.
- **Testing**: Future focused tests must cover cycle, limit, backend-error, malformed-proof, and successful traversal cases.
